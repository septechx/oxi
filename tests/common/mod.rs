use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use oxic::context::{with_ctx, with_ctx_mut};
use oxic::driver::compile_source;
use oxic::errors::ErrorLevel;

static TEST_RUN_ID: AtomicUsize = AtomicUsize::new(0);

pub struct Test {
    files: Vec<(String, String)>,
    should_succeed: Option<bool>,
    fail_on_level: ErrorLevel,
    expected_errors: Vec<String>,
}

impl Test {
    pub fn new() -> Self {
        with_ctx_mut(|ctx| {
            ctx.errors.clear();
            ctx.errors.set_panic_on_fatal(false);
        });

        Self {
            files: vec![],
            should_succeed: None,
            fail_on_level: ErrorLevel::Error,
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

    pub fn expect_error(&mut self, error: &str) -> &mut Self {
        self.expected_errors.push(error.to_string());
        self
    }

    /// Returns `true` if the test result does not match expectations.
    fn check_for_errors(&self) -> bool {
        with_ctx(|ctx| {
            let has_error = ctx.errors.has_errors_above_level(self.fail_on_level);

            let missing_expected_error = self
                .expected_errors
                .iter()
                .any(|code| !ctx.errors.has_code(code));

            if has_error {
                ctx.errors.print_errors(ErrorLevel::Warning);
            }

            match (has_error, self.should_succeed == Some(false)) {
                (false, false) => missing_expected_error,
                (false, true) => true,
                (true, false) => true,
                (true, true) => missing_expected_error,
            }
        })
    }

    /// Abort pipeline only on:
    /// - All expected errors already found (early success — no need to continue)
    /// - Unexpected errors in a "should succeed" test
    /// - Missing expected errors after some errors occurred
    ///
    /// Does NOT abort on `(false, true)` — errors may be emitted by later stages.
    fn should_abort(&self) -> bool {
        with_ctx(|ctx| {
            if self.should_succeed == Some(false)
                && !self.expected_errors.is_empty()
                && self
                    .expected_errors
                    .iter()
                    .all(|code| ctx.errors.has_code(code))
            {
                return true;
            }

            let has_error = ctx.errors.has_errors_above_level(self.fail_on_level);
            if !has_error {
                return false;
            }

            let missing_expected = self
                .expected_errors
                .iter()
                .any(|code| !ctx.errors.has_code(code));

            match self.should_succeed == Some(false) {
                true => missing_expected,
                false => true,
            }
        })
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

        let mut root_source = None;
        for (filename, content) in &self.files {
            let file_path = test_dir.join(filename);
            if let Some(parent) = file_path.parent()
                && let Err(e) = fs::create_dir_all(parent)
            {
                panic!("Failed to create directory: {}", e);
            }
            fs::write(&file_path, content).unwrap();
            if filename == "main.oxi" || filename.ends_with("/main.oxi") {
                root_source = Some((file_path, content.clone()));
            }
        }

        let root = root_source.expect("test must define main.oxi");
        let res = compile_source(
            root.0,
            root.1,
            || {
                if self.should_abort() {
                    Err(anyhow::anyhow!("pipeline aborted"))
                } else {
                    Ok(())
                }
            },
            None,
        );

        if let Err(e) = &res
            && self.should_succeed != Some(false)
            && !with_ctx(|ctx| ctx.errors.has_errors_above_level(self.fail_on_level))
        {
            panic!("Compiler driver failed without reporting diagnostics: {e}");
        }

        if self.check_for_errors() {
            if res.is_err()
                && self.expected_errors.is_empty()
                && !with_ctx(|ctx| ctx.errors.has_errors_above_level(self.fail_on_level))
            {
                return;
            }
            panic!(
                "Test failed: expected diagnostic errors {:?} were not satisfied",
                self.expected_errors
            );
        }
    }
}

pub fn with(f: impl FnOnce(&mut Test)) {
    let mut test = Test::new();
    f(&mut test);
}
