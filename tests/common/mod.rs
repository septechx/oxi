use std::fmt::Display;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use thin_vec::ThinVec;

use oxic::ast::validate::validate_ast;
use oxic::context::{with_ctx, with_ctx_mut};
use oxic::errors::ErrorLevel;
use oxic::hir::AstLoweringContext;
use oxic::lexer::tokenize;
use oxic::parser::parse;
use oxic::resolve::{Resolver, build_module_tree};
use oxic::thir::lower_thir;
use oxic::thir::scope::build_scope_trees;
use oxic::typeck::typeck_crate;

static TEST_RUN_ID: AtomicUsize = AtomicUsize::new(0);

pub struct Test {
    files: Vec<(String, String)>,
    should_succeed: Option<bool>,
    fail_on_level: ErrorLevel,
    expected_errors: Vec<String>,
}

impl Test {
    pub fn new() -> Self {
        with_ctx_mut(|ctx| ctx.errors.clear());

        Self {
            files: vec![],
            should_succeed: None,
            fail_on_level: ErrorLevel::Warning,
            expected_errors: vec![],
        }
    }

    pub fn add_source(&mut self, filename: &str, source: &str) -> &mut Self {
        self.files
            .push((filename.to_string(), source.trim().to_string()));
        self
    }

    pub fn succeeds(&mut self, should: bool) -> &mut Self {
        self.should_succeed = Some(should);
        self
    }

    pub fn fail_on_level(&mut self, level: ErrorLevel) -> &mut Self {
        self.fail_on_level = level;
        self
    }

    pub fn expect_error(&mut self, error: &str) -> &mut Self {
        self.expected_errors.push(error.to_string());
        self
    }

    fn check_for_errors(&self) -> bool {
        with_ctx(|ctx| {
            let has_error = ctx.errors.has_errors_above_level(self.fail_on_level);

            // If specific error codes were expected, they must all be present.
            let missing_expected_error = self
                .expected_errors
                .iter()
                .any(|code| !ctx.errors.has_code(code));

            if has_error {
                ctx.errors.print_errors(ErrorLevel::Warning);
            }

            match (has_error, self.should_succeed == Some(false)) {
                // success, expected success
                (false, false) => false,

                // success, expected failure
                (false, true) => true,

                // failure, expected success
                (true, false) => true,

                // failure, expected failure
                (true, true) => missing_expected_error,
            }
        })
    }

    fn checkpoint(&self) -> Result<(), ()> {
        if self.check_for_errors() {
            Err(())
        } else {
            Ok(())
        }
    }

    fn hard_check<T, E: Display>(&self, result: Result<T, E>, msg: &str) -> Result<T, ()> {
        match result {
            Ok(v) => Ok(v),
            Err(e) => {
                if self.should_succeed == Some(false) {
                    // We expected failure, but this bypassed ctx.errors.
                    // If the test expected specific diagnostics, they can never appear.
                    if self.expected_errors.is_empty() {
                        return Err(());
                    }

                    panic!(
                        "{}: {} (expected diagnostic errors {:?}, but execution failed before they were emitted)",
                        msg, e, self.expected_errors
                    );
                }

                panic!("{}: {}", msg, e);
            }
        }
    }

    fn run_pipeline(&mut self, file_paths: &[PathBuf]) -> Result<(), ()> {
        let mut asts = ThinVec::new();
        for file_path in file_paths {
            let source =
                self.hard_check(fs::read_to_string(file_path), "Failed to read source file")?;
            let (tokens, module_id) =
                self.hard_check(tokenize(source, file_path), "Tokenization failed")?;
            self.checkpoint()?;

            let ast = self.hard_check(parse(tokens, file_path), "Parsing failed")?;
            self.checkpoint()?;

            validate_ast(&ast, module_id);
            self.checkpoint()?;

            asts.push(ast);
        }

        with_ctx_mut(|ctx| {
            Resolver::assign_node_ids(ctx, &mut asts);
        });
        self.checkpoint()?;

        let module_tree = self.hard_check(
            build_module_tree(&asts, file_paths, "main"),
            "build_module_tree failed",
        )?;
        self.checkpoint()?;

        let resolver = with_ctx_mut(|ctx| {
            let mut resolver = Resolver::new(&asts, &module_tree, ctx);
            resolver.resolve();
            resolver.into_resolver_outputs()
        });
        self.checkpoint()?;

        let mut hir_crate = with_ctx_mut(|ctx| {
            let mut lowering_ctx = AstLoweringContext::new(ctx, &asts, &module_tree, &resolver);
            lowering_ctx.lower_crate()
        });
        self.checkpoint()?;

        let typeck = with_ctx_mut(|ctx| typeck_crate(ctx, &mut hir_crate, &resolver));
        self.checkpoint()?;
        typeck.assert_no_errors();

        let scope_trees = build_scope_trees(&hir_crate);
        let _thir_crate = lower_thir(&hir_crate, &typeck, &scope_trees);
        self.checkpoint()?;

        Ok(())
    }
}

impl Drop for Test {
    fn drop(&mut self) {
        let temp_dir = PathBuf::from(".oxi/tests");
        let mut hasher = DefaultHasher::new();
        self.files.hash(&mut hasher);
        let hash = format!("{:016x}", hasher.finish());
        let run_id = TEST_RUN_ID.fetch_add(1, Ordering::Relaxed);
        let test_dir = temp_dir.join(format!("{hash}-{run_id}"));

        if let Err(e) = fs::create_dir_all(&test_dir) {
            panic!("Failed to create test directory: {}", e);
        }

        let mut file_paths = Vec::new();
        for (filename, content) in &self.files {
            let file_path = test_dir.join(filename);
            if let Some(parent) = file_path.parent()
                && let Err(e) = fs::create_dir_all(parent)
            {
                panic!("Failed to create directory: {}", e);
            }
            if let Err(e) = fs::write(&file_path, content) {
                panic!("Failed to write source file {}: {}", filename, e);
            }
            file_paths.push(file_path);
        }

        let res = self.run_pipeline(&file_paths);
        if self.should_succeed == Some(false) && res.is_ok() {
            panic!("Expected pipeline to fail, but it succeeded");
        }
    }
}

pub fn with(f: impl FnOnce(&mut Test)) {
    let mut test = Test::new();
    f(&mut test);
}
