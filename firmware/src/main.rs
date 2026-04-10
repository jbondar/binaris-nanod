mod com;
mod haptic;
mod motor;
mod pins;
mod thread;

fn main() {
    // Link ESP-IDF patches (required for std support)
    esp_idf_svc::sys::link_patches();

    // Initialize logging
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("NanoD firmware starting");

    // Spawn FOC control thread on Core 1
    thread::foc_thread::spawn_foc_thread();

    // Spawn COM thread on Core 0
    com::com_thread::spawn_com_thread();

    log::info!("All threads spawned");

    // Main thread idle — work happens in FOC and COM threads
    loop {
        unsafe {
            esp_idf_sys::vTaskDelay(1000);
        }
    }
}
