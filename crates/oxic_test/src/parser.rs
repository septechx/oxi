use anyhow::Result;

pub struct TestCase {
    pub auxiliary_modules: Vec<String>,
    pub expected_errors: Vec<String>,
}

pub fn parse_file(input: &str) -> Result<TestCase> {
    let mut auxiliary_modules = Vec::new();
    let mut expected_errors = Vec::new();
    for (idx, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || (idx == 0 && line.starts_with("#!")) {
            continue;
        }
        if line.starts_with("// @auxiliary-module") {
            let rest = line.strip_prefix("// @auxiliary-module").unwrap();
            auxiliary_modules.push(rest.trim().to_string());
        } else if line.starts_with("// @expect-error") {
            let rest = line.strip_prefix("// @expect-error").unwrap();
            expected_errors.push(rest.trim().to_string());
        } else {
            break;
        }
    }

    Ok(TestCase {
        auxiliary_modules,
        expected_errors,
    })
}
