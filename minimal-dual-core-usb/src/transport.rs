use core::str::from_utf8;

use defmt::{info, warn};
use embassy_futures::select::select;
use embassy_rp::usb;
use embassy_usb::class::cdc_acm::CdcAcmClass;
use heapless::Vec;
use minimal_usb_protocol::{HostToSensor, SensorToHost, CONFIG_VERSION};

use crate::{usb_device::Disconnected, ID, INCOMING, OUTGOING};

/// Deserializes bytes off the usb device, forwards the messages on [`INCOMING`].
/// Serializes outgoing packets from [`OUTGOING`] into the usb device.
pub async fn transport<'d, T: usb::Instance + 'd>(
    class: &mut CdcAcmClass<'d, usb::Driver<'d, T>>,
) -> Result<(), Disconnected> {
    use embassy_futures::{
        select::Either::First as IncomingBytes, select::Either::Second as OutgoingMessage,
    };
    let pong: Vec<u8, 1> = postcard::to_vec(&SensorToHost::Pong).expect("It is valid");
    let me: Vec<u8, 32> = postcard::to_vec(&SensorToHost::Id {
        name: ID,
        version: CONFIG_VERSION,
    })
    .expect("It is valid");
    let mut buf = [0; 128];
    loop {
        match select(class.read_packet(&mut buf), OUTGOING.receive()).await {
            IncomingBytes(result) => {
                let n = result?;
                let data = &buf[..n];
                let message: HostToSensor = match postcard::from_bytes(data) {
                    Ok(msg) => msg,
                    Err(e) => {
                        warn!(
                            "Invalid bytes: {:x} ({:?}). Error: {}",
                            data,
                            from_utf8(data).unwrap_or("non-ascii"),
                            e
                        );
                        continue;
                    }
                };
                info!("received message: {:x}", message);
                match message {
                    HostToSensor::WhoAreYou => {
                        info!("got WhoAreYou, identifying");
                        class.write_packet(&me).await?;
                    }
                    HostToSensor::Ping => {
                        info!("got ping, sending pong");
                        class.write_packet(&pong).await?;
                    }
                    msg => {
                        info!("got {:?}", msg);
                        INCOMING.send(msg).await;
                    }
                }
            }
            OutgoingMessage(msg) => {
                info!("got message, forwarding it: {}", msg);
                let bytes: Vec<u8, 128> = postcard::to_vec(&msg).unwrap();
                class.write_packet(&bytes).await?;
            }
        }
    }
}
