use clap::{Parser, Subcommand};

#[cfg(unix)]
const DEFAULT_TTY: &str = "/dev/ttyACM1";
#[cfg(windows)]
const DEFAULT_TTY: &str = "COM1";

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Arguments {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    ListAvailablePorts,

    Connect {
        #[arg(default_value = DEFAULT_TTY)]
        tty: String,
    },
}
