use std::io::{BufRead, BufReader, Read, Write};

use esp_idf_sys::*;
use nanod_math::protocol::serialize;

use super::dispatch::{Action, Dispatcher};
use crate::ipc::{ComContext, DisplayCommand};

const COM_STACK_SIZE: usize = 32768; // Larger for base64 album art decoding
const COM_PRIORITY: u32 = 1;
const COM_CORE: i32 = 0;
// FreeRTOS tick rate is 100Hz (10ms/tick). vTaskDelay takes ticks, not ms.
const LOOP_DELAY_TICKS: u32 = 1; // 1 tick = 10ms

/// Spawn the COM thread on Core 0.
pub fn spawn_com_thread(ctx: ComContext) {
    let ctx_ptr = Box::into_raw(Box::new(ctx)) as *mut core::ffi::c_void;
    unsafe {
        let mut handle: TaskHandle_t = core::ptr::null_mut();
        xTaskCreatePinnedToCore(
            Some(com_task),
            b"com\0".as_ptr() as *const _,
            COM_STACK_SIZE as u32,
            ctx_ptr,
            COM_PRIORITY,
            &mut handle,
            COM_CORE,
        );
    }
}

unsafe extern "C" fn com_task(arg: *mut core::ffi::c_void) {
    let ctx = unsafe { *Box::from_raw(arg as *mut ComContext) };
    if let Err(e) = com_task_inner(ctx) {
        log::error!("COM task failed: {:?}", e);
    }
    loop {
        vTaskDelay(1000);
    }
}

fn com_task_inner(ctx: ComContext) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("COM thread starting");

    let mut dispatcher = Dispatcher::new();

    // ESP-IDF USB CDC maps to stdin/stdout
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = std::io::stdout();

    let mut line_buf = String::new();

    log::info!("COM thread ready, listening for JSON commands");

    loop {
        // Read more data into the buffer.
        // Don't clear — accumulate until we have a complete line (\n).
        // Large JSON messages arrive in multiple USB packets; read_line
        // may return a partial line when the USB buffer is temporarily empty.
        match reader.read_line(&mut line_buf) {
            Ok(0) => {
                if !line_buf.is_empty() {
                    // Have partial data — busy-wait briefly for more USB packets
                    unsafe { esp_rom_delay_us(100) };
                } else {
                    // Truly idle — normal sleep
                    unsafe { vTaskDelay(LOOP_DELAY_TICKS) };
                }
            }
            Ok(_) => {
                // Only process when we have a complete line (ends with \n)
                if !line_buf.ends_with('\n') {
                    // Partial read — immediately try for more
                    continue;
                }

                let line = line_buf.trim();
                if line.is_empty() {
                    line_buf.clear();
                    continue;
                }

                log::debug!("COM rx: {}", &line[..line.len().min(80)]);

                let actions = dispatcher.handle_line(line);
                line_buf.clear();
                for action in actions {
                    match action {
                        Action::Respond(json) => {
                            let _ = stdout.write_all(json.as_bytes());
                            let _ = stdout.write_all(b"\n");
                            let _ = stdout.flush();
                        }
                        Action::UpdateHaptic(profile) => {
                            log::info!(
                                "COM→FOC: haptic update — {} detents, {:?} mode",
                                profile.detent_count,
                                profile.mode
                            );
                            let _ = ctx
                                .foc_tx
                                .try_send(crate::ipc::FocCommand::UpdateHaptic(profile));
                        }
                        Action::UpdateLedConfig(led_config) => {
                            log::info!("COM→HMI: LED config update");
                            let _ = ctx
                                .hmi_tx
                                .try_send(crate::ipc::HmiCommand::UpdateLedConfig(led_config));
                        }
                        Action::UpdateSettings {
                            brightness,
                            orientation,
                        } => {
                            log::info!("COM→HMI: settings update");
                            let _ = ctx.hmi_tx.try_send(crate::ipc::HmiCommand::UpdateSettings {
                                brightness,
                                orientation,
                            });
                        }
                        Action::Recalibrate => {
                            log::info!("COM→FOC: recalibrate");
                            let _ = ctx.foc_tx.try_send(crate::ipc::FocCommand::Recalibrate);
                        }
                        Action::Display(display_cmd) => {
                            let desc = match &display_cmd {
                                crate::ipc::DisplayCommand::SetMode(m) => format!("SetMode({:?})", m),
                                crate::ipc::DisplayCommand::MediaMeta { title, .. } => format!("MediaMeta({})", title),
                                crate::ipc::DisplayCommand::MediaArtChunk { offset, .. } => format!("ArtChunk@{}", offset),
                                crate::ipc::DisplayCommand::MediaArtStart => "ArtStart".to_string(),
                                crate::ipc::DisplayCommand::MediaArtDone => "ArtDone".to_string(),
                            };
                            match ctx.display_tx.try_send(display_cmd) {
                                Ok(()) => log::info!("COM→Display: {}", desc),
                                Err(e) => log::error!("COM→Display FAILED: {} — {}", desc, e),
                            }
                        }
                        Action::BinaryArtReceive(size) => {
                            // Tell display to hide canvas during transfer
                            let _ = ctx.display_tx.try_send(DisplayCommand::MediaArtStart);
                            // ACK — tell host to start sending raw bytes
                            let _ = stdout.write_all(b"{\"ack\":\"art_bin\"}\n");
                            let _ = stdout.flush();
                            // Read through BufReader to keep its buffer in sync
                            receive_binary_art(
                                &mut reader,
                                &ctx.display_tx,
                                size as usize,
                            );
                        }
                        Action::Save => {
                            save_profiles_to_spiffs(&dispatcher);
                        }
                        Action::None => {}
                    }
                }
            }
            Err(e) => {
                // EAGAIN (os error 11) is normal when no USB CDC is connected
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    log::warn!("COM read error: {e}");
                }
                unsafe { vTaskDelay(LOOP_DELAY_TICKS) };
            }
        }

        // Forward key events from HMI to serial output
        while let Ok(key_event) = ctx.key_rx.try_recv() {
            if let Ok(json) = serialize::serialize_event(&serialize::key_event(
                key_event.key.num,
                &key_event.key.state,
            )) {
                let _ = stdout.write_all(json.as_bytes());
                let _ = stdout.write_all(b"\n");
                let _ = stdout.flush();
            }
        }
    }
}

/// Read raw binary album art data from stdin directly into the shared ART_BUF.
/// Bypasses channels entirely — writes to the static buffer, then signals display thread.
fn receive_binary_art(
    stdin: &mut impl Read,
    display_tx: &std::sync::mpsc::SyncSender<DisplayCommand>,
    size: usize,
) {
    use crate::display::display_thread::{ART_LOAD_PTR, ART_BUF_SIZE};

    let buf_ptr = unsafe { ART_LOAD_PTR };
    if buf_ptr.is_null() {
        log::error!("ART_BUF not allocated");
        return;
    }

    let max = size.min(ART_BUF_SIZE);
    let mut received = 0usize;

    while received < max {
        let dest = unsafe {
            core::slice::from_raw_parts_mut(buf_ptr.add(received), max - received)
        };
        match stdin.read(dest) {
            Ok(0) => {
                unsafe { esp_rom_delay_us(50) };
            }
            Ok(n) => {
                received += n;
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    log::error!("binary art read error: {e}");
                    return;
                }
                unsafe { esp_rom_delay_us(50) };
            }
        }
    }

    // Signal display thread to refresh the canvas
    let _ = display_tx.try_send(DisplayCommand::MediaArtDone);
    log::info!("Binary art received: {} bytes", received);
}

fn save_profiles_to_spiffs(dispatcher: &Dispatcher) {
    let dirty = dispatcher.profiles.dirty_profiles();
    for profile in dirty {
        let path = format!("/spiffs/profiles/{}.json", profile.name);
        match serde_json::to_string(profile) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, &json) {
                    log::error!("failed to write {path}: {e}");
                } else {
                    log::info!("saved profile '{}' to SPIFFS", profile.name);
                }
            }
            Err(e) => log::error!("failed to serialize profile '{}': {e}", profile.name),
        }
    }

    // Save settings
    match serde_json::to_string(&dispatcher.settings) {
        Ok(json) => {
            if let Err(e) = std::fs::write("/spiffs/device_settings.json", &json) {
                log::error!("failed to write settings: {e}");
            } else {
                log::info!("saved device settings to SPIFFS");
            }
        }
        Err(e) => log::error!("failed to serialize settings: {e}"),
    }
}
