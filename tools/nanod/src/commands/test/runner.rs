/// Tracks test pass/fail results across suites.
pub struct TestResults {
    passed: Vec<String>,
    failed: Vec<(String, String)>,
    skipped: Vec<String>,
}

impl TestResults {
    pub fn new() -> Self {
        Self {
            passed: Vec::new(),
            failed: Vec::new(),
            skipped: Vec::new(),
        }
    }

    pub fn pass(&mut self, name: &str) {
        println!("  [PASS] {name}");
        self.passed.push(name.to_string());
    }

    pub fn fail(&mut self, name: &str, reason: &str) {
        println!("  [FAIL] {name}: {reason}");
        self.failed.push((name.to_string(), reason.to_string()));
    }

    pub fn skip(&mut self, name: &str, reason: &str) {
        println!("  [SKIP] {name}: {reason}");
        self.skipped.push(name.to_string());
    }

    pub fn print_summary(&self) {
        let total = self.passed.len() + self.failed.len() + self.skipped.len();
        println!("\n--- Test Summary ---");
        println!(
            "{} passed, {} failed, {} skipped (of {total})",
            self.passed.len(),
            self.failed.len(),
            self.skipped.len()
        );

        if !self.failed.is_empty() {
            println!("\nFailures:");
            for (name, reason) in &self.failed {
                println!("  - {name}: {reason}");
            }
        }

        if self.failed.is_empty() && !self.passed.is_empty() {
            println!("\nAll tests passed!");
        }
    }
}

/// Prompt the user for a yes/no confirmation. Used for physical validation tests.
pub fn prompt_user(msg: &str) -> bool {
    use std::io::{self, Write};
    print!("  ? {msg} [y/n]: ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().eq_ignore_ascii_case("y")
}
