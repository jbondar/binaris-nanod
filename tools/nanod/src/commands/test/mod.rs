mod runner;
pub(crate) mod serial_proto;

mod haptic;
mod motor;
mod serial;

// Placeholder suites for future phases
mod audio;
mod buttons;
mod display;
mod leds;

use anyhow::Result;

pub fn run(suite: &str, baud: Option<u32>, port: Option<&str>) -> Result<()> {
    let baud = baud.unwrap_or(115200);

    println!("NanoD Hardware Test Runner");
    println!("========================\n");

    let (port_name, _usb_info) = crate::device::connection::resolve_port(port)?;
    println!("Using port: {port_name}");
    println!("Baud rate:  {baud}\n");

    let mut proto = serial_proto::SerialProto::open(&port_name, baud)?;

    let mut results = runner::TestResults::new();

    match suite {
        "motor" => motor::run_suite(&mut proto, &mut results)?,
        "haptic" => haptic::run_suite(&mut proto, &mut results)?,
        "serial" => serial::run_suite(&mut proto, &mut results)?,
        "buttons" => buttons::run_suite(&mut proto, &mut results)?,
        "leds" => leds::run_suite(&mut proto, &mut results)?,
        "display" => display::run_suite(&mut proto, &mut results)?,
        "audio" => audio::run_suite(&mut proto, &mut results)?,
        "all" => {
            serial::run_suite(&mut proto, &mut results)?;
            motor::run_suite(&mut proto, &mut results)?;
            haptic::run_suite(&mut proto, &mut results)?;
            buttons::run_suite(&mut proto, &mut results)?;
            leds::run_suite(&mut proto, &mut results)?;
            display::run_suite(&mut proto, &mut results)?;
            audio::run_suite(&mut proto, &mut results)?;
        }
        _ => {
            anyhow::bail!(
                "Unknown suite '{suite}'. Available: motor, haptic, serial, buttons, leds, display, audio, all"
            );
        }
    }

    results.print_summary();
    Ok(())
}
