use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::device::connection;
use crate::device::constants::*;

pub fn run(baud: Option<u32>, port: Option<&str>) -> Result<()> {
    let baud = baud.unwrap_or(MONITOR_BAUD);
    let port_name = connection::resolve_port(port)?;

    println!("Opening serial monitor on {} at {} baud", port_name, baud);
    println!("Press Ctrl+C to exit\n");

    let mut serial = serialport::new(&port_name, baud)
        .timeout(Duration::from_millis(100))
        .open()
        .with_context(|| format!("Failed to open {}", port_name))?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc_handler(r);

    // Read thread: serial -> stdout
    let mut reader = serial.try_clone().context("Failed to clone serial port")?;
    let r2 = running.clone();
    let read_handle = thread::spawn(move || {
        let mut buf = [0u8; 1024];
        let stdout = io::stdout();
        while r2.load(Ordering::Relaxed) {
            match reader.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let mut out = stdout.lock();
                    let _ = out.write_all(&buf[..n]);
                    let _ = out.flush();
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {}
                Err(_) => break,
            }
        }
    });

    // Main thread: stdin -> serial
    let stdin = io::stdin();
    let mut stdin_buf = [0u8; 256];
    while running.load(Ordering::Relaxed) {
        match stdin.lock().read(&mut stdin_buf) {
            Ok(n) if n > 0 => {
                let _ = serial.write_all(&stdin_buf[..n]);
            }
            _ => {}
        }
    }

    let _ = read_handle.join();
    println!("\nMonitor closed.");
    Ok(())
}

fn ctrlc_handler(running: Arc<AtomicBool>) {
    let _ = ctrlc::set_handler(move || {
        running.store(false, Ordering::Relaxed);
    });
}
