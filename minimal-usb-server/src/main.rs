use std::str::from_utf8;

use arguments::{Arguments, Command};
use bytes::Bytes;
use clap::Parser;
use futures::{stream::StreamExt, SinkExt};
use minimal_usb_protocol::*;
use tokio::{
    select,
    sync::mpsc::{self, error::SendError},
    task::spawn_blocking,
    try_join,
};
use tokio_serial::SerialPortBuilderExt;
use tokio_util::codec::{BytesCodec, Decoder};

mod arguments;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Arguments::parse();

    match args.command {
        Command::ListAvailablePorts => {
            let ports = tokio_serial::available_ports()?;
            let info = serde_yaml::to_string(&ports)?;
            println!("{info}");
        }
        Command::Connect { tty } => {
            cfg_if::cfg_if! {
                if #[cfg(unix)] {
                    use anyhow::Context;
                    let mut port = tokio_serial::new(&tty, 115_200).open_native_async()?;
                    port.set_exclusive(false)
                        .context("Unable to set serial port exclusive to false")?;
                } else {
                    let port = tokio_serial::new(&tty, 115_200).open_native_async()?;
                }
            };

            println!("Connection established on '{tty}'.");

            let port = BytesCodec::new().framed(port);
            let (mut writer, mut reader) = port.split::<Bytes>();

            let (tx, mut rx) = mpsc::channel::<HostToSensor>(32);

            let input_handler = spawn_blocking(move || forward_stdin_to_channel(tx));

            loop {
                select! {
                    Some(msg) = rx.recv() => {
                        let bytes = match postcard::to_vec::<_, 64>(&msg).map(|s| s.to_vec()) {
                            Ok(bytes) => bytes,
                            Err(err) => {
                                println!("Found invalid bytes, skipping sending: {err}");
                                continue;
                            }
                        };
                        let Ok(()) = writer.send(bytes.into()).await else {
                            break;
                        };
                    }
                    Some(Ok(msg)) = reader.next() => {
                        let item = match postcard::from_bytes::<SensorToHost>(&msg) {
                            Ok(item) => item,
                            Err(err) => {
                                println!("Found invalid bytes, skipping processing: {err}");
                                continue;
                            }
                        };
                        if let Err(e) = process_item(item) {
                            println!("Error while processing item: {e:?}");
                            continue;
                        }
                    }
                    else => break,
                }
            }
            try_join!(input_handler)?.0?;
        }
    }

    Ok(())
}

fn process_item(item: SensorToHost) -> anyhow::Result<()> {
    match item {
        SensorToHost::Id { name, version } => {
            println!(
                "Device name: {} (version: {version})",
                from_utf8(&name).unwrap()
            );
        }
        SensorToHost::Pong => {
            println!("Received Pong");
        }
        SensorToHost::Config(config) => {
            println!("Got config: {config:#?}");
        }
        SensorToHost::ConfigOk => {
            println!("Received config OK");
        }
    }
    Ok(())
}

fn forward_stdin_to_channel(tx: mpsc::Sender<HostToSensor>) -> anyhow::Result<()> {
    let mut buffer = String::new();
    let stdin = std::io::stdin();

    loop {
        buffer.clear();
        let n = match stdin.read_line(&mut buffer) {
            Ok(0) => {
                println!("Stdin closed.");
                break Ok(());
            }
            Ok(n) => n,
            Err(err) => {
                println!("Failed to read line: {err:?}");
                break Ok(());
            }
        };
        let Ok(message) = serde_yaml::from_str::<HostToSensor>(&buffer[..n]) else {
            println!(
                "Invalid input: {:?}. Try Ping, WhoAreYou, Trigger, or GetConfig",
                &buffer[..n]
            );
            continue;
        };
        if let Err(SendError(msg)) = tx.blocking_send(message) {
            println!("Failed to send {msg:?}, exiting");
            break Ok(());
        }
    }
}
