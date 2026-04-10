use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serialport::SerialPort;

/// JSON-over-serial protocol helper.
/// Sends newline-terminated JSON commands and reads newline-terminated JSON responses.
pub struct SerialProto {
    port: Box<dyn SerialPort>,
    reader: BufReader<Box<dyn SerialPort>>,
}

impl SerialProto {
    pub fn open(port_name: &str, baud: u32) -> Result<Self> {
        let port = serialport::new(port_name, baud)
            .timeout(Duration::from_millis(100))
            .open()
            .with_context(|| format!("failed to open {port_name}"))?;

        let reader_port = port
            .try_clone()
            .context("failed to clone serial port for reader")?;

        Ok(Self {
            port,
            reader: BufReader::new(reader_port),
        })
    }

    /// Send a JSON command (newline-terminated).
    pub fn send(&mut self, json: &str) -> Result<()> {
        let mut msg = json.trim().to_string();
        msg.push('\n');
        self.port
            .write_all(msg.as_bytes())
            .context("serial write failed")?;
        self.port.flush().context("serial flush failed")?;
        Ok(())
    }

    /// Read one JSON line with timeout.
    pub fn read_line(&mut self, timeout: Duration) -> Result<Option<String>> {
        let start = Instant::now();
        let mut buf = String::new();

        loop {
            buf.clear();
            match self.reader.read_line(&mut buf) {
                Ok(0) => {}
                Ok(_) => {
                    let line = buf.trim().to_string();
                    if !line.is_empty() {
                        return Ok(Some(line));
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => bail!("serial read error: {e}"),
            }

            if start.elapsed() >= timeout {
                return Ok(None);
            }
        }
    }

    /// Send a command and wait for a JSON response.
    pub fn send_and_recv(
        &mut self,
        json: &str,
        timeout: Duration,
    ) -> Result<Option<serde_json::Value>> {
        self.send(json)?;
        match self.read_line(timeout)? {
            Some(line) => {
                let val: serde_json::Value =
                    serde_json::from_str(&line).context("invalid JSON response")?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    /// Drain all pending data from the serial port (clear buffer).
    pub fn drain(&mut self) -> Result<()> {
        loop {
            match self.read_line(Duration::from_millis(50))? {
                Some(_) => continue,
                None => break,
            }
        }
        Ok(())
    }

    /// Collect all responses within a time window.
    pub fn collect_responses(&mut self, duration: Duration) -> Result<Vec<serde_json::Value>> {
        let start = Instant::now();
        let mut responses = Vec::new();
        while start.elapsed() < duration {
            if let Some(line) = self.read_line(Duration::from_millis(100))? {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                    responses.push(val);
                }
            }
        }
        Ok(responses)
    }
}
