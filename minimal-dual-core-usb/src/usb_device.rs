use embassy_rp::{peripherals::USB, usb};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    driver::EndpointError,
    Builder, Config as UsbConfig, UsbDevice,
};
use static_cell::StaticCell;

static USB_DEVICE: StaticCell<UsbDevice<'static, usb::Driver<'static, USB>>> = StaticCell::new();
static USB_CLASS: StaticCell<CdcAcmClass<'static, usb::Driver<'static, USB>>> = StaticCell::new();
//static USB_SENDER: cdc_acm::Sender<'static, usb::Driver<'static, USB>> = StaticCell::new();
static USB_STATE: StaticCell<State<'static>> = StaticCell::new();

static DEVICE_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 128]> = StaticCell::new();

use crate::Irqs;

pub struct Disconnected;

pub const USB_CONFIG: UsbConfig<'static> = {
    let mut config = UsbConfig::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Nope");
    config.product = Some("Nope");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;
    config
};

pub fn initialize(
    usb: USB,
    irqs: Irqs,
) -> (
    &'static mut UsbDevice<'static, usb::Driver<'static, USB>>,
    &'static mut CdcAcmClass<'static, usb::Driver<'static, USB>>,
) {
    // Create the driver, from the HAL.
    let usb_driver = usb::Driver::new(usb, irqs);

    let device_descriptor_buf = DEVICE_DESCRIPTOR.init([0; 256]);
    let config_descriptor_buf = CONFIG_DESCRIPTOR.init([0; 256]);
    let bos_descriptor_buf = BOS_DESCRIPTOR.init([0; 256]);
    let control_buf = CONTROL_BUF.init([0; 128]);

    // Create embassy-usb DeviceBuilder using the driver and config.
    let mut builder = Builder::new(
        usb_driver,
        USB_CONFIG,
        device_descriptor_buf,
        config_descriptor_buf,
        bos_descriptor_buf,
        &mut [],
        control_buf,
    );

    // It needs some buffers for building the descriptors.
    let state = USB_STATE.init(State::new());

    // Create classes on the builder.
    let class = USB_CLASS.init(CdcAcmClass::new(&mut builder, state, 64));

    // Build the builder.
    let device = USB_DEVICE.init(builder.build());
    (device, class)
}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected,
        }
    }
}
