use std::time::Duration;

use anyhow::Result;

use super::runner::{self, TestResults};
use super::serial_proto::SerialProto;

const TIMEOUT: Duration = Duration::from_secs(5);

pub fn run_suite(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    println!("\n[motor] Motor & FOC Tests");
    println!("-------------------------");
    println!("  These tests require physical interaction with the device.\n");

    proto.drain()?;

    test_recalibrate(proto, results)?;
    test_encoder_events(proto, results)?;
    test_detent_response(proto, results)?;

    Ok(())
}

fn test_recalibrate(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "motor::recalibrate";
    match proto.send_and_recv(r#"{"motor": {"recalibrate": true}}"#, TIMEOUT)? {
        Some(val) => {
            if let Some(msg) = val.get("msg") {
                let text = msg["text"].as_str().unwrap_or("");
                if text.contains("recalibrat") {
                    results.pass(name);
                } else {
                    results.fail(name, &format!("unexpected: {text}"));
                }
            } else {
                results.fail(name, &format!("no msg: {val}"));
            }
        }
        None => results.fail(name, "no response"),
    }
    Ok(())
}

fn test_encoder_events(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "motor::encoder_events";

    // Load a profile so angle events fire
    proto.send(r#"{"profile": {"name": "enc_test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}"#)?;
    proto.drain()?;

    println!("  >> Slowly rotate the knob back and forth for 5 seconds...");
    let events = proto.collect_responses(Duration::from_secs(5))?;

    let angle_events: Vec<_> = events.iter().filter(|e| e.get("angle").is_some()).collect();
    if angle_events.len() >= 2 {
        results.pass(name);
    } else {
        results.fail(
            name,
            &format!(
                "expected >= 2 angle events, got {} (of {} total events)",
                angle_events.len(),
                events.len()
            ),
        );
    }
    Ok(())
}

fn test_detent_response(_proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "motor::detent_feel";

    if runner::prompt_user("Do you feel distinct haptic detents when rotating the knob?") {
        results.pass(name);
    } else {
        results.fail(name, "user reports no haptic detents felt");
    }
    Ok(())
}
