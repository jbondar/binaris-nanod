use anyhow::Result;

use super::runner::TestResults;
use super::serial_proto::SerialProto;

pub fn run_suite(_proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    println!("\n[buttons] Button Input Tests");
    println!("----------------------------");
    println!("  (Phase 3 — not yet implemented)\n");

    results.skip("buttons::press_events", "not yet implemented");
    results.skip("buttons::hold_states", "not yet implemented");

    Ok(())
}
