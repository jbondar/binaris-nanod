use anyhow::Result;

use super::runner::TestResults;
use super::serial_proto::SerialProto;

pub fn run_suite(_proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    println!("\n[audio] Audio Output Tests");
    println!("--------------------------");
    println!("  (Phase 5 — not yet implemented)\n");

    results.skip("audio::playback", "not yet implemented");

    Ok(())
}
