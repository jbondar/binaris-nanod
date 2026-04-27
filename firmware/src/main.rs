mod com;
mod display;
mod haptic;
mod hmi;
mod ipc;
mod motor;
mod pins;
mod thread;

use std::sync::mpsc;

use display::display_thread::DisplayContext;
use ipc::{ComContext, FocContext, HmiContext};

fn main() {
    // Link ESP-IDF patches (required for std support)
    esp_idf_svc::sys::link_patches();

    // Initialize logging
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("NanoD firmware starting");

    // Allocate double-buffered album art on heap (PSRAM, not DRAM)
    let art_display = vec![0u8; display::display_thread::ART_BUF_SIZE];
    let art_load = vec![0u8; display::display_thread::ART_BUF_SIZE];
    unsafe {
        display::display_thread::ART_DISPLAY_PTR =
            Box::into_raw(art_display.into_boxed_slice()) as *mut u8;
        display::display_thread::ART_LOAD_PTR =
            Box::into_raw(art_load.into_boxed_slice()) as *mut u8;
    };

    // --- Create inter-thread channels ---
    let (foc_cmd_tx, foc_cmd_rx) = mpsc::sync_channel(4);
    let (angle_tx, angle_rx) = mpsc::sync_channel(1);
    let (display_angle_tx, display_angle_rx) = mpsc::sync_channel(1);
    let (hmi_cmd_tx, hmi_cmd_rx) = mpsc::sync_channel(4);
    let (key_tx, key_rx) = mpsc::sync_channel(8);
    let (display_cmd_tx, display_cmd_rx) = mpsc::sync_channel(8);

    // --- Spawn threads ---
    thread::foc_thread::spawn_foc_thread(FocContext {
        cmd_rx: foc_cmd_rx,
        angle_tx,
        display_tx: display_angle_tx,
    });

    com::com_thread::spawn_com_thread(ComContext {
        foc_tx: foc_cmd_tx,
        hmi_tx: hmi_cmd_tx,
        display_tx: display_cmd_tx,
        key_rx,
    });

    hmi::hmi_thread::spawn_hmi_thread(HmiContext {
        angle_rx,
        cmd_rx: hmi_cmd_rx,
        key_tx,
    });

    display::display_thread::spawn_display_thread(DisplayContext {
        angle_rx: display_angle_rx,
        cmd_rx: display_cmd_rx,
    });

    log::info!("All threads spawned");

    // Main thread idle — work happens in FOC, COM, HMI, and display threads
    loop {
        unsafe {
            esp_idf_sys::vTaskDelay(1000);
        }
    }
}
