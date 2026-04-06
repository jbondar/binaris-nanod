use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serialport::SerialPortType;

use crate::device::constants::*;

/// Find the NanoD serial port by VID/PID.
pub fn find_nanod_port() -> Result<String> {
    let ports = serialport::available_ports().context("Failed to enumerate serial ports")?;

    for port in &ports {
        if let SerialPortType::UsbPort(usb) = &port.port_type {
            if usb.vid == USB_VID && USB_PIDS.contains(&usb.pid) {
                log::info!("Found NanoD on {}", port.port_name);
                return Ok(port.port_name.clone());
            }
        }
    }

    bail!(
        "NanoD not found. Is the device connected?\nAvailable ports: {}",
        ports
            .iter()
            .map(|p| p.port_name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Resolve port: use user-specified port or auto-detect.
pub fn resolve_port(port: Option<&str>) -> Result<String> {
    match port {
        Some(p) => Ok(p.to_string()),
        None => find_nanod_port(),
    }
}

/// Enter bootloader via 1200bps touch (DTR toggle).
/// The ESP32-S3 USB CDC resets into download mode when a 1200 baud
/// connection opens and asserts DTR.
pub fn enter_bootloader(port_name: &str) -> Result<()> {
    log::info!("Entering bootloader via 1200bps touch on {}", port_name);

    let mut port = serialport::new(port_name, 1200)
        .timeout(Duration::from_secs(1))
        .open()
        .context("Failed to open port for bootloader entry")?;

    port.write_data_terminal_ready(true)
        .context("Failed to assert DTR")?;

    // Brief pause then close — the USB CDC will reset
    thread::sleep(Duration::from_millis(250));
    drop(port);

    // Wait for device to re-enumerate
    log::info!("Waiting for device to re-enumerate...");
    thread::sleep(Duration::from_secs(2));

    Ok(())
}

/// Wait for a serial port to appear (polling with timeout).
pub fn wait_for_port(timeout: Duration) -> Result<String> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(port) = find_nanod_port() {
            return Ok(port);
        }
        thread::sleep(Duration::from_millis(500));
    }
    bail!("Timed out waiting for device to appear")
}
