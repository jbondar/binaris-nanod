use anyhow::Result;

use super::runner::TestResults;
use super::serial_proto::SerialProto;

pub fn run_suite(_proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    println!("\n[display] LCD Display Tests");
    println!("---------------------------");
    println!("  (Phase 4 — not yet implemented)\n");

    results.skip("display::screen_layout", "not yet implemented");

    Ok(())
}
