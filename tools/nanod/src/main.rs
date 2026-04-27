mod cli;
mod commands;
mod device;
mod util;
mod validate;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { firmware } => {
            commands::validate::run(&firmware)?;
        }
        Commands::Flash {
            firmware,
            skip_validate,
        } => {
            commands::flash::run(&firmware, skip_validate, cli.port.as_deref())?;
        }
        Commands::Backup { output } => {
            commands::backup::run(&output, cli.port.as_deref())?;
        }
        Commands::Restore { image, force } => {
            commands::restore::run(&image, force, cli.port.as_deref())?;
        }
        Commands::Recover { firmware } => {
            commands::recover::run(&firmware, cli.port.as_deref())?;
        }
        Commands::Monitor { baud } => {
            commands::monitor::run(baud, cli.port.as_deref())?;
        }
        Commands::Media { baud } => {
            commands::media::run(baud, cli.port.as_deref())?;
        }
        Commands::Test { suite, baud } => {
            commands::test::run(&suite, baud, cli.port.as_deref())?;
        }
    }

    Ok(())
}
