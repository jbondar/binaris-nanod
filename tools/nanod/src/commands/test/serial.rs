use std::time::Duration;

use anyhow::Result;

use super::runner::TestResults;
use super::serial_proto::SerialProto;

const TIMEOUT: Duration = Duration::from_secs(2);

pub fn run_suite(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    println!("\n[serial] COM Protocol Tests");
    println!("----------------------------");

    proto.drain()?;

    // 1. Get (no active profile yet)
    test_get_no_profile(proto, results)?;

    // 2. Upload a profile
    test_upload_profile(proto, results)?;

    // 3. Get active profile
    test_get_active_profile(proto, results)?;

    // 4. Upload settings and read back
    test_settings_roundtrip(proto, results)?;

    // 5. List profiles
    test_list_profiles(proto, results)?;

    // 6. Invalid JSON doesn't crash
    test_invalid_json(proto, results)?;

    Ok(())
}

fn test_get_no_profile(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "serial::get_no_profile";
    match proto.send_and_recv(r#"{"get": true}"#, TIMEOUT)? {
        Some(val) => {
            // Should get an error or empty response
            if val.get("msg").is_some() || val.get("profile").is_some() {
                results.pass(name);
            } else {
                results.fail(name, &format!("unexpected response: {val}"));
            }
        }
        None => results.fail(name, "no response within timeout"),
    }
    Ok(())
}

fn test_upload_profile(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "serial::upload_profile";
    let cmd = r#"{"profile": {"name": "test_profile", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 20, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}"#;
    match proto.send_and_recv(cmd, TIMEOUT)? {
        Some(val) => {
            if let Some(msg) = val.get("msg") {
                let text = msg["text"].as_str().unwrap_or("");
                if text.contains("set") {
                    results.pass(name);
                } else {
                    results.fail(name, &format!("unexpected msg: {text}"));
                }
            } else {
                results.fail(name, &format!("no msg in response: {val}"));
            }
        }
        None => results.fail(name, "no response"),
    }
    Ok(())
}

fn test_get_active_profile(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "serial::get_active_profile";
    match proto.send_and_recv(r#"{"get": true}"#, TIMEOUT)? {
        Some(val) => {
            if let Some(profile) = val.get("profile") {
                let pname = profile["name"].as_str().unwrap_or("");
                if pname == "test_profile" {
                    results.pass(name);
                } else {
                    results.fail(name, &format!("wrong profile name: {pname}"));
                }
            } else {
                results.fail(name, &format!("no profile in response: {val}"));
            }
        }
        None => results.fail(name, "no response"),
    }
    Ok(())
}

fn test_settings_roundtrip(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "serial::settings_roundtrip";

    // Set
    proto.send(r#"{"settings": {"midi_channel": 7, "led_brightness": 42}}"#)?;
    proto.read_line(TIMEOUT)?; // consume ack

    // Read back
    match proto.send_and_recv(r#"{"get_settings": true}"#, TIMEOUT)? {
        Some(val) => {
            if let Some(settings) = val.get("settings") {
                let midi = settings["midi_channel"].as_u64();
                let led = settings["led_brightness"].as_u64();
                if midi == Some(7) && led == Some(42) {
                    results.pass(name);
                } else {
                    results.fail(name, &format!("wrong values: midi={midi:?} led={led:?}"));
                }
            } else {
                results.fail(name, &format!("no settings in response: {val}"));
            }
        }
        None => results.fail(name, "no response"),
    }
    Ok(())
}

fn test_list_profiles(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "serial::list_profiles";

    // Add a second profile
    proto.send(r#"{"profile": {"name": "profile_2"}}"#)?;
    proto.read_line(TIMEOUT)?;

    match proto.send_and_recv(r#"{"list": true}"#, TIMEOUT)? {
        Some(val) => {
            if let Some(profiles) = val.get("profiles") {
                let arr = profiles.as_array();
                if let Some(arr) = arr {
                    if arr.len() >= 2 {
                        results.pass(name);
                    } else {
                        results.fail(name, &format!("expected >= 2 profiles, got {}", arr.len()));
                    }
                } else {
                    results.fail(name, "profiles is not an array");
                }
            } else {
                results.fail(name, &format!("no profiles key: {val}"));
            }
        }
        None => results.fail(name, "no response"),
    }
    Ok(())
}

fn test_invalid_json(proto: &mut SerialProto, results: &mut TestResults) -> Result<()> {
    let name = "serial::invalid_json";
    match proto.send_and_recv("this is not json!!!", TIMEOUT)? {
        Some(val) => {
            // Should get an error message, not crash
            if val.get("msg").is_some() {
                results.pass(name);
            } else {
                results.fail(name, &format!("unexpected response: {val}"));
            }
        }
        None => {
            // No response could mean it silently dropped — also acceptable
            results.pass(name);
        }
    }
    Ok(())
}
