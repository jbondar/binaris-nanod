mod com;
mod haptic;
mod hmi;
mod ipc;
mod motor;
mod pins;
mod thread;

use std::sync::mpsc;

use ipc::{ComContext, FocContext, HmiContext};

fn main() {
    // Link ESP-IDF patches (required for std support)
    esp_idf_svc::sys::link_patches();

    // Initialize logging
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("NanoD firmware starting");

    // --- Create inter-thread channels ---
    let (foc_cmd_tx, foc_cmd_rx) = mpsc::sync_channel(4);
    let (angle_tx, angle_rx) = mpsc::sync_channel(1);
    let (hmi_cmd_tx, hmi_cmd_rx) = mpsc::sync_channel(4);
    let (key_tx, key_rx) = mpsc::sync_channel(8);

    // --- Spawn threads ---
    thread::foc_thread::spawn_foc_thread(FocContext {
        cmd_rx: foc_cmd_rx,
        angle_tx,
    });

    com::com_thread::spawn_com_thread(ComContext {
        foc_tx: foc_cmd_tx,
        hmi_tx: hmi_cmd_tx,
        key_rx,
    });

    hmi::hmi_thread::spawn_hmi_thread(HmiContext {
        angle_rx,
        cmd_rx: hmi_cmd_rx,
        key_tx,
    });

    log::info!("All threads spawned");

    // Main thread idle — work happens in FOC, COM, and HMI threads
    loop {
        unsafe {
            esp_idf_sys::vTaskDelay(1000);
        }
    }
}
