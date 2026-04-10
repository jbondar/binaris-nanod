use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nanod", about = "NanoD/Ratchet_H1 device tools")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Serial port (auto-detected if omitted)
    #[arg(short, long, global = true)]
    pub port: Option<String>,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Validate a firmware binary without flashing
    Validate {
        /// Path to firmware.bin
        firmware: PathBuf,
    },

    /// Flash firmware to the app0 partition
    Flash {
        /// Path to firmware.bin
        firmware: PathBuf,

        /// Skip pre-flash validation
        #[arg(long)]
        skip_validate: bool,
    },

    /// Backup full flash contents to a file
    Backup {
        /// Output file path
        output: PathBuf,
    },

    /// Restore a full flash image (overwrites everything)
    Restore {
        /// Path to flash image
        image: PathBuf,

        /// Required flag to confirm destructive operation
        #[arg(long)]
        force: bool,
    },

    /// Flash firmware when device is already in ROM download mode
    Recover {
        /// Path to firmware.bin
        firmware: PathBuf,
    },

    /// Open a serial monitor
    Monitor {
        /// Baud rate (default: 115200)
        #[arg(short, long)]
        baud: Option<u32>,
    },

    /// Run hardware test suites over serial
    Test {
        /// Test suite to run (motor, haptic, serial, buttons, leds, display, audio, all)
        #[arg(default_value = "all")]
        suite: String,

        /// Baud rate (default: 115200)
        #[arg(short, long)]
        baud: Option<u32>,
    },
}
