use std::time::Duration;

use anyhow::Result;

use super::runner::{self, TestResults};
use super::serial_proto::SerialProto;

pub fn run_suite(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    println!("\n[haptic] Haptic Detent Tests");
    println!("----------------------------");
    println!("  These tests require physical interaction with the device.\n");

    proto.drain()?;

    test_default_profile_detents(proto, results)?;
    test_endstop_events(proto, results)?;
    test_vernier_mode(proto, results)?;
    test_profile_switch(proto, results)?;

    Ok(())
}

fn test_default_profile_detents(
    proto: &mut SerialProto,
    results: &mut TestResults,
) -> Result<()> {
    let name = "haptic::default_detents";

    // Load default-like profile: 60 detents
    proto.send(r#"{"profile": {"name": "default_test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}"#)?;
    proto.drain()?;

    println!("  >> Rotate the knob slowly through several detents...");
    let events = proto.collect_responses(Duration::from_secs(5))?;
    let angle_count = events.iter().filter(|e| e.get("angle").is_some()).count();

    if angle_count >= 3 {
        results.pass(name);
    } else {
        results.fail(
            name,
            &format!("expected >= 3 position events, got {angle_count}"),
        );
    }
    Ok(())
}

fn test_endstop_events(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "haptic::endstop";

    // Small range profile: 0-10, so endstops are easy to reach
    proto.send(r#"{"profile": {"name": "endstop_test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 10, "detent_count": 10, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 5.0}}}"#)?;
    proto.drain()?;

    println!("  >> Rotate the knob until you feel a hard endstop, then keep pushing...");
    let events = proto.collect_responses(Duration::from_secs(5))?;

    // Check for any events (ideally we'd check for limit events)
    if !events.is_empty() {
        if runner::prompt_user("Did you feel a firm endstop that prevented further rotation?") {
            results.pass(name);
        } else {
            results.fail(name, "user reports no endstop felt");
        }
    } else {
        results.fail(name, "no events received");
    }
    Ok(())
}

fn test_vernier_mode(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "haptic::vernier_mode";

    proto.send(r#"{"profile": {"name": "vernier_test", "haptic": {"mode": "vernier", "start_pos": 0, "end_pos": 20, "detent_count": 20, "vernier": 5, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}"#)?;
    proto.drain()?;

    println!("  >> Rotate the knob slowly — you should feel fine detents between coarse ones...");

    if runner::prompt_user("Do the detents feel finer/closer together than the previous test?") {
        results.pass(name);
    } else {
        results.fail(name, "user reports vernier detents not felt");
    }
    Ok(())
}

fn test_profile_switch(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "haptic::profile_switch";

    // Load profile A (few detents)
    proto.send(r#"{"profile": {"name": "switch_A", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 10, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}"#)?;
    proto.drain()?;

    println!("  >> Feel the current detent spacing (wide apart)...");
    std::thread::sleep(Duration::from_secs(2));

    // Switch to profile B (many detents)
    proto.send(r#"{"profile": {"name": "switch_B", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}"#)?;
    proto.drain()?;

    println!("  >> Now feel the detents again (should be closer together)...");

    if runner::prompt_user("Did the detent spacing change noticeably between the two profiles?") {
        results.pass(name);
    } else {
        results.fail(name, "user reports no difference in detent spacing");
    }
    Ok(())
}
