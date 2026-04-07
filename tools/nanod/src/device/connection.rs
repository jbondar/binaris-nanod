use std::time::Duration;

use anyhow::{bail, Context, Result};
use espflash::connection::{Connection, ResetAfterOperation, ResetBeforeOperation};
use espflash::flasher::Flasher;
use serialport::{SerialPortType, UsbPortInfo};

use crate::device::constants::*;

/// Find the NanoD serial port by VID/PID.
pub fn find_nanod_port() -> Result<(String, UsbPortInfo)> {
    let ports = serialport::available_ports().context("Failed to enumerate serial ports")?;

    for port in &ports {
        if let SerialPortType::UsbPort(usb) = &port.port_type {
            if usb.vid == USB_VID && USB_PIDS.contains(&usb.pid) {
                log::info!("Found NanoD on {}", port.port_name);
                return Ok((port.port_name.clone(), usb.clone()));
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
/// Returns the port name and USB port info.
pub fn resolve_port(port: Option<&str>) -> Result<(String, UsbPortInfo)> {
    match port {
        Some(p) => {
            // User specified a port, look up its USB info
            let ports =
                serialport::available_ports().context("Failed to enumerate serial ports")?;
            for pi in &ports {
                if pi.port_name == p {
                    if let SerialPortType::UsbPort(usb) = &pi.port_type {
                        return Ok((p.to_string(), usb.clone()));
                    }
                }
            }
            // If not found as USB, create a dummy UsbPortInfo
            Ok((
                p.to_string(),
                UsbPortInfo {
                    vid: USB_VID,
                    pid: USB_PIDS[0],
                    serial_number: None,
                    manufacturer: None,
                    product: None,
                },
            ))
        }
        None => find_nanod_port(),
    }
}

/// Connect to the device and return a Flasher instance.
/// This handles opening the serial port, creating the espflash Connection,
/// and initializing the Flasher (which enters bootloader, loads stub, etc).
pub fn connect_flasher(
    port: Option<&str>,
    reset_before: ResetBeforeOperation,
) -> Result<Flasher> {
    let (port_name, usb_info) = resolve_port(port)?;
    println!("Using port: {}", port_name);

    let serial = serialport::new(&port_name, 115_200)
        .timeout(Duration::from_secs(3))
        .open_native()
        .with_context(|| format!("Failed to open serial port: {}", port_name))?;

    let connection = Connection::new(
        serial,
        usb_info,
        ResetAfterOperation::HardReset,
        reset_before,
        115_200,
    );

    let flasher = Flasher::connect(
        connection,
        true,  // use_stub: faster flashing via RAM stub
        true,  // verify: verify after write
        true,  // skip: skip already-flashed regions
        None,  // chip: auto-detect
        Some(UPLOAD_BAUD), // baud: switch to fast baud after connect
    )
    .context("Failed to connect to device. Is it in bootloader mode?")?;

    Ok(flasher)
}
