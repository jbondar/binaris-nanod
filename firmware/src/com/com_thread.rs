use std::io::{BufRead, BufReader, Write};

use esp_idf_sys::*;

use super::dispatch::{Action, Dispatcher};

const COM_STACK_SIZE: usize = 8192;
const COM_PRIORITY: u32 = 1;
const COM_CORE: i32 = 0;
const LOOP_DELAY_MS: u32 = 10; // 100Hz

/// Spawn the COM thread on Core 0.
pub fn spawn_com_thread() {
    unsafe {
        let mut handle: TaskHandle_t = core::ptr::null_mut();
        xTaskCreatePinnedToCore(
            Some(com_task),
            b"com\0".as_ptr() as *const _,
            COM_STACK_SIZE as u32,
            core::ptr::null_mut(),
            COM_PRIORITY,
            &mut handle,
            COM_CORE,
        );
    }
}

unsafe extern "C" fn com_task(_arg: *mut core::ffi::c_void) {
    if let Err(e) = com_task_inner() {
        log::error!("COM task failed: {:?}", e);
    }
    loop {
        vTaskDelay(1000);
    }
}

fn com_task_inner() -> Result<(), Box<dyn std::error::Error>> {
    log::info!("COM thread starting");

    let mut dispatcher = Dispatcher::new();

    // ESP-IDF USB CDC maps to stdin/stdout
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = std::io::stdout();

    let mut line_buf = String::new();

    log::info!("COM thread ready, listening for JSON commands");

    loop {
        line_buf.clear();

        // Non-blocking: try to read a line
        match reader.read_line(&mut line_buf) {
            Ok(0) => {
                // No data / EOF — sleep and retry
                unsafe { vTaskDelay(LOOP_DELAY_MS) };
                continue;
            }
            Ok(_) => {
                let line = line_buf.trim();
                if line.is_empty() {
                    continue;
                }

                log::debug!("COM rx: {}", line);

                let actions = dispatcher.handle_line(line);
                for action in actions {
                    match action {
                        Action::Respond(json) => {
                            let _ = stdout.write_all(json.as_bytes());
                            let _ = stdout.flush();
                        }
                        Action::UpdateHaptic(profile) => {
                            // TODO: send to FOC thread via queue
                            log::info!(
                                "haptic update: {} detents, {:?} mode",
                                profile.detent_count,
                                profile.mode
                            );
                        }
                        Action::Recalibrate => {
                            // TODO: send recalibrate command to FOC thread
                            log::info!("motor recalibration requested");
                        }
                        Action::Save => {
                            save_profiles_to_spiffs(&dispatcher);
                        }
                        Action::None => {}
                    }
                }
            }
            Err(e) => {
                log::warn!("COM read error: {e}");
                unsafe { vTaskDelay(LOOP_DELAY_MS) };
            }
        }
    }
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
