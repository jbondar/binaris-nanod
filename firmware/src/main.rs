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

    log::info!("FOC thread spawned, main thread idle");

    // Main thread: placeholder for future COM thread (serial protocol, etc.)
    loop {
        unsafe {
            esp_idf_sys::vTaskDelay(1000);
        }
    }
}
