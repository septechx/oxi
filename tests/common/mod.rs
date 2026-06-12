use oxic::{
    ast::validate::validate_ast,
    context::{with_ctx, with_ctx_mut},
    errors::ErrorLevel,
    hir::AstLoweringContext,
    lexer::tokenize,
    parser::parse,
    resolve::{Resolver, build_module_tree},
    typeck::typeck_crate,
};
use std::{
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use thin_vec::ThinVec;

static TEST_RUN_ID: AtomicUsize = AtomicUsize::new(0);

pub struct Test {
    files: Vec<(String, String)>,
    should_succeed: Option<bool>,
    fail_on_level: ErrorLevel,
}

impl Test {
    pub fn new() -> Self {
        with_ctx_mut(|ctx| ctx.errors.clear());

        Self {
            files: vec![],
            should_succeed: None,
            fail_on_level: ErrorLevel::Warning,
        }
    }

    /// Add a source file with a given filename. The filename should be relative
    /// The first file added is assumed to be the crate root (must be named main.oxi).
    pub fn add_source(&mut self, filename: &str, source: &str) -> &mut Self {
        self.files
            .push((filename.to_string(), source.trim().to_string()));
        self
    }

    /// Expect resolution to succeed (true) or fail (false).
    /// If not set, panics on any error above fail_on_level.
    pub fn succeeds(&mut self, should: bool) -> &mut Self {
        self.should_succeed = Some(should);
        self
    }

    /// Set the error level at which the test should fail.
    pub fn fail_on_level(&mut self, level: ErrorLevel) -> &mut Self {
        self.fail_on_level = level;
        self
    }

    fn check_for_errors(&self) -> bool {
        with_ctx(|ctx| {
            if ctx.errors.has_errors_above_level(self.fail_on_level) {
                ctx.errors.print_errors(ErrorLevel::Warning);
                return true;
            }
            false
        })
    }

    fn handle_error_check(&self) -> bool {
        if self.check_for_errors() {
            if self.should_succeed == Some(false) {
                return true;
            }
            panic!("Resolution had errors or warnings above threshold");
        }
        false
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

        // Phase 1: Tokenize and parse all files
        let mut asts = ThinVec::new();
        for file_path in &file_paths {
            let source = match fs::read_to_string(file_path) {
                Ok(s) => s,
                Err(e) => {
                    if self.should_succeed == Some(false) {
                        return;
                    }
                    panic!("Failed to read source file: {}", e);
                }
            };

            let (tokens, module_id) = match tokenize(source, file_path) {
                Ok(t) => t,
                Err(_) => {
                    if self.should_succeed == Some(false) {
                        return;
                    }
                    panic!("Tokenization failed");
                }
            };

            if self.handle_error_check() {
                return;
            }

            let ast = match parse(tokens, file_path) {
                Ok(a) => a,
                Err(_) => {
                    if self.should_succeed == Some(false) {
                        return;
                    }
                    panic!("Parsing failed");
                }
            };

            if self.handle_error_check() {
                return;
            }

            validate_ast(&ast, module_id);
            if self.handle_error_check() {
                return;
            }

            asts.push(ast);
        }

        with_ctx_mut(|ctx| {
            Resolver::assign_node_ids(ctx, &mut asts);
        });

        // Phase 2: Build module tree
        let module_tree = match build_module_tree(&asts, &file_paths, "main") {
            Ok(tree) => tree,
            Err(_) => {
                if self.should_succeed == Some(false) {
                    return;
                }
                panic!("Module tree building failed");
            }
        };

        if self.handle_error_check() {
            return;
        }

        // Phase 3: Run name resolution
        let resolver = with_ctx_mut(|ctx| {
            let mut resolver = Resolver::new(&asts, &module_tree, ctx);
            resolver.resolve();
            resolver.into_resolver_outputs()
        });

        if self.handle_error_check() {
            return;
        }

        // Phase 4: Lower to HIR
        let mut hir_crate = with_ctx_mut(|ctx| {
            let mut lowering_ctx = AstLoweringContext::new(ctx, &asts, &module_tree, &resolver);
            lowering_ctx.lower_crate()
        });

        if self.handle_error_check() {
            return;
        }

        // Phase 5: Type check
        let typeck = with_ctx_mut(|ctx| typeck_crate(ctx, &mut hir_crate, &resolver));
        if self.should_succeed == Some(false) {
            if !self.check_for_errors() {
                panic!("Expected a type error but none occurred");
            }
            return;
        }
        self.handle_error_check();
        typeck.assert_no_errors();
    }
}

pub fn with(f: impl FnOnce(&mut Test)) {
    let mut test = Test::new();
    f(&mut test);
}
