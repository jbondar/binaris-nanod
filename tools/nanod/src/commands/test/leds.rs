use anyhow::Result;

use super::runner::TestResults;
use super::serial_proto::SerialProto;

pub fn run_suite(_proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    println!("\n[leds] LED Output Tests");
    println!("-----------------------");
    println!("  (Phase 3 — not yet implemented)\n");

    results.skip("leds::color_config", "not yet implemented");
    results.skip("leds::color_cycle", "not yet implemented");

    Ok(())
}
