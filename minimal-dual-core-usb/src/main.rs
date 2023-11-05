#![cfg_attr(not(test), no_std)]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt::info;
use embassy_executor::Executor;
use embassy_rp::{
    bind_interrupts,
    config::Config,
    gpio::{Input, Level, Output, Pull},
    i2c::{self, I2c},
    multicore::{spawn_core1, Stack},
    peripherals::{I2C0, PIN_25, PIN_4, PIN_5, USB},
    usb::{self},
    watchdog::Watchdog,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::Timer;
use embassy_usb::{class::cdc_acm::CdcAcmClass, UsbDevice};
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::{DrawTarget, Point},
    text::{Alignment, Text},
    Drawable,
};
use formatter::Formatter;
use minimal_usb_protocol::{HostToSensor, SensorToHost, CONFIG_VERSION};
use ssd1306::{
    mode::BufferedGraphicsMode,
    prelude::{DisplayConfig, I2CInterface},
    rotation::DisplayRotation,
    size::DisplaySize128x64,
    Ssd1306,
};
use static_cell::StaticCell;

use crate::usb_device::initialize;
use {defmt_rtt as _, panic_probe as _};

use crate::transport::transport;

use core::fmt::Write;

mod formatter;
mod transport;
mod usb_device;

// Global constants.
pub const ID: [u8; 24] = *b"Minimal USB device\0\0\0\0\0\0";

pub const STYLE: MonoTextStyle<'_, BinaryColor> = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);

// Global channels.
pub static OUTGOING: Channel<CriticalSectionRawMutex, SensorToHost, 128> =
    Channel::<CriticalSectionRawMutex, SensorToHost, 128>::new();

pub static INCOMING: Channel<CriticalSectionRawMutex, HostToSensor, 128> =
    Channel::<CriticalSectionRawMutex, HostToSensor, 128>::new();

// Executors.
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static mut CORE1_STACK: Stack<65_536> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

// Bind interrupts to the handlers inside embassy.
bind_interrupts!(struct Irqs {
    I2C0_IRQ => i2c::InterruptHandler<I2C0>;
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

#[cortex_m_rt::entry]
fn main() -> ! {
    info!("Hello there!");

    let peripherals = embassy_rp::init(Config::default());

    let mut i2c0_config = i2c::Config::default();
    i2c0_config.frequency = 400_000;

    let i2c0_sda = peripherals.PIN_0;
    let i2c0_scl = peripherals.PIN_1;

    let oled_reset = Output::new(peripherals.PIN_4, Level::Low);

    // Start pin @ DSub #10.
    let start = Input::new(peripherals.PIN_5, Pull::None);

    let led = Output::new(peripherals.PIN_25, Level::Low);

    let i2c0_bus = i2c::I2c::new_async(peripherals.I2C0, i2c0_scl, i2c0_sda, Irqs, i2c0_config);
    let display_interface = ssd1306::I2CDisplayInterface::new(i2c0_bus);
    let display = Ssd1306::new(
        display_interface,
        DisplaySize128x64,
        DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();

    let watchdog = Watchdog::new(peripherals.WATCHDOG);

    let (usb_device, usb_class) = initialize(peripherals.USB, Irqs);

    spawn_core1(peripherals.CORE1, unsafe { &mut CORE1_STACK }, move || {
        let executor1 = EXECUTOR1.init(Executor::new());
        executor1.run(|spawner| {
            spawner
                .spawn(do_things(oled_reset, display, start, led, watchdog))
                .unwrap();
        })
    });

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        spawner.spawn(run_usb_device(usb_device)).unwrap();
        spawner.spawn(run_protocol(usb_class)).unwrap();
    });
}

#[embassy_executor::task]
pub async fn do_things(
    mut oled_reset: Output<'static, PIN_4>,
    mut display: Ssd1306<
        I2CInterface<I2c<'static, I2C0, i2c::Async>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    mut start: Input<'static, PIN_5>,
    mut led: Output<'static, PIN_25>,
    watchdog: Watchdog,
) -> ! {
    oled_reset.set_high();
    oled_reset.set_low();
    Timer::after_millis(10).await;
    oled_reset.set_high();

    display.init().expect("Display connected?");
    display.clear(BinaryColor::Off).unwrap();
    display.flush().unwrap();

    let mut buf: Formatter<96> = Formatter::new();

    let mut counter = 0;
    loop {
        start.wait_for_high().await;
        led.set_high();
        display.clear(BinaryColor::Off).unwrap();
        display.flush().unwrap();
        info!("doing things {}", counter);
        write!(buf, "Counter: {counter}").unwrap();
        OUTGOING
            .send(SensorToHost::Id {
                name: ID,
                version: CONFIG_VERSION,
            })
            .await;
        Text::with_alignment(buf.as_str(), Point::new(64, 15), STYLE, Alignment::Center)
            .draw(&mut display)
            .unwrap();
        display.flush().unwrap();
        counter += 1;
        buf.clear();
        start.wait_for_low().await;
        led.set_low();
    }
}

#[embassy_executor::task]
pub async fn run_usb_device(
    usb_device: &'static mut UsbDevice<'static, usb::Driver<'static, USB>>,
) {
    usb_device.run().await;
}

#[embassy_executor::task]
pub async fn run_protocol(class: &'static mut CdcAcmClass<'static, usb::Driver<'static, USB>>) {
    loop {
        class.wait_connection().await;
        info!("Connected");
        let _ = transport(class).await;
        info!("Disconnected");
    }
}
