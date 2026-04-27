mod controller;
mod image_convert;
mod media_source;

use anyhow::Result;

use crate::device::connection::resolve_port;
use crate::device::constants::MONITOR_BAUD;

pub fn run(baud: Option<u32>, port: Option<&str>) -> Result<()> {
    let baud = baud.unwrap_or(MONITOR_BAUD);
    let (port_name, _) = resolve_port(port)?;

    println!("NanoD Media Controller");
    println!("Port: {port_name} @ {baud} baud");
    println!("Press Ctrl+C to exit\n");

    controller::run_media_loop(&port_name, baud)
}
