use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use base64::Engine;

use super::image_convert;
use super::media_source::{self, MediaSource, NowPlaying};
use crate::commands::test::serial_proto::SerialProto;

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const ART_CHUNK_SIZE: usize = 57600; // ~77KB base64 — 2 chunks for full image, fast over USB

pub fn run_media_loop(port_name: &str, baud: u32) -> Result<()> {
    let mut proto = SerialProto::open(port_name, baud)?;
    let mut source = media_source::create_media_source();

    // Handle Ctrl+C gracefully
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })?;

    // Enter media mode — send commands with proper settling
    proto.send(r#"{"media_mode": "on"}"#)?;
    std::thread::sleep(Duration::from_millis(500));
    proto.drain()?;

    proto.send(r#"{"media_haptic": "volume"}"#)?;
    std::thread::sleep(Duration::from_millis(200));
    proto.drain()?;

    println!("Media mode active. Polling for Now Playing...");

    let mut last_track: Option<(String, String)> = None; // (title, album)
    let mut last_poll = Instant::now();

    while running.load(Ordering::Relaxed) {
        // Read device events (button presses, angle changes)
        handle_device_events(&mut proto, source.as_mut())?;

        // Poll media source periodically
        if last_poll.elapsed() >= POLL_INTERVAL {
            last_poll = Instant::now();

            match source.get_now_playing() {
                Ok(Some(np)) => {
                    let track_key = (np.title.clone(), np.album.clone());
                    let track_changed = last_track.as_ref() != Some(&track_key);

                    // Send album art FIRST on track change (before metadata)
                    // so LVGL doesn't redraw canvas during transfer
                    if track_changed {
                        println!("{} - {}", np.artist, np.title);

                        let t0 = Instant::now();
                        let art_sent = if let Some(url) = &np.artwork_url {
                            // Request 300x300 from Spotify CDN (faster than 640x640)
                            let small_url = url.replace("ab67616d0000b273", "ab67616d00004851");
                            match send_album_art_from_url(&mut proto, &small_url) {
                                Ok(()) => true,
                                Err(e) => {
                                    eprintln!("  Art fetch failed: {e}");
                                    false
                                }
                            }
                        } else {
                            false
                        };
                        if !art_sent {
                            let _ = send_test_art(&mut proto);
                        }
                        println!("  Art loaded in {:.1}s", t0.elapsed().as_secs_f32());
                        last_track = Some(track_key);
                    }

                    // Send metadata after art so labels update with art visible
                    send_media_meta(&mut proto, &np)?;
                }
                Ok(None) => {
                    // No media playing
                }
                Err(e) => {
                    log::warn!("Media source error: {e}");
                }
            }
        }

        std::thread::sleep(Duration::from_millis(50));
    }

    // Exit media mode and restore default haptic profile
    println!("\nExiting media mode...");
    proto.send(r#"{"media_mode": "off"}"#)?;
    std::thread::sleep(Duration::from_millis(100));
    // Restore default profile: 60 detents, 0-255, matches C++ firmware
    proto.send(r#"{"profile":{"name":"default","haptic":{"mode":"regular","start_pos":0,"end_pos":255,"detent_count":60,"vernier":5,"output_ramp":0.0,"detent_strength":3.0}}}"#)?;

    Ok(())
}

fn send_media_meta(proto: &mut SerialProto, np: &NowPlaying) -> Result<()> {
    let cmd = serde_json::json!({
        "media_meta": {
            "title": np.title,
            "artist": np.artist,
            "album": np.album,
            "duration": np.duration_s,
            "position": np.position_s,
            "playing": np.playing
        }
    });
    proto.send(&cmd.to_string())?;
    Ok(())
}

fn send_album_art_from_url(proto: &mut SerialProto, url: &str) -> Result<()> {
    let t0 = Instant::now();
    let rgb565_data = image_convert::fetch_and_convert_artwork(url)
        .context("artwork conversion failed")?;
    let fetch_time = t0.elapsed();

    let t1 = Instant::now();
    send_art_data(proto, &rgb565_data)?;
    let send_time = t1.elapsed();

    eprintln!("  fetch+convert: {:.1}s, serial send: {:.1}s",
        fetch_time.as_secs_f32(), send_time.as_secs_f32());
    Ok(())
}

fn send_test_art(proto: &mut SerialProto) -> Result<()> {
    let rgb565_data = image_convert::generate_test_pattern();
    send_art_data(proto, &rgb565_data)
}

fn send_art_data(proto: &mut SerialProto, rgb565_data: &[u8]) -> Result<()> {
    // Use binary transfer: send command, wait for ACK, then raw bytes
    let cmd = format!(r#"{{"media_art_bin":{}}}"#, rgb565_data.len());
    proto.send(&cmd)?;

    // Wait for device ACK (skip any interleaved button/angle events)
    let deadline = Instant::now() + Duration::from_millis(2000);
    loop {
        match proto.read_line(Duration::from_millis(200))? {
            Some(line) if line.contains("ack") => break,
            Some(_) => continue, // skip button/angle events
            None => {
                if Instant::now() >= deadline {
                    anyhow::bail!("no ACK for media_art_bin");
                }
            }
        }
    }

    // Send raw binary data directly to serial port
    use std::io::Write;
    proto.write_raw(rgb565_data)?;

    println!("  Album art sent ({} bytes, binary)", rgb565_data.len());
    Ok(())
}

fn handle_device_events(proto: &mut SerialProto, source: &mut dyn MediaSource) -> Result<()> {
    while let Ok(Some(line)) = proto.read_line(Duration::from_millis(10)) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
            // Handle button events
            if let Some(key) = val.get("key") {
                let num = key.get("num").and_then(|v| v.as_u64()).unwrap_or(99) as u8;
                let state = key
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if state == "pressed" {
                    match num {
                        0 => {
                            // Button A: Play/Pause
                            println!("  [Play/Pause]");
                            let _ = source.play_pause();
                        }
                        1 => {
                            // Button B: Toggle volume/scrub mode
                            println!("  [Mode toggle] (not yet implemented)");
                        }
                        2 => {
                            // Button C: Previous
                            println!("  [Previous]");
                            let _ = source.prev_track();
                        }
                        3 => {
                            // Button D: Next
                            println!("  [Next]");
                            let _ = source.next_track();
                        }
                        _ => {}
                    }
                }
            }

            // Handle angle events for volume control
            if let Some(angle) = val.get("angle") {
                if let Some(pos) = angle.get("cur_pos").and_then(|v| v.as_u64()) {
                    // Map position (0-100 in volume mode) to volume percentage
                    let vol = (pos as f32).clamp(0.0, 100.0);
                    let _ = source.set_volume(vol);
                }
            }
        }
    }
    Ok(())
}
