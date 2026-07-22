//! Static-lifetime composite USB HID construction.

use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_usb::class::hid::{
    Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State as HidState,
};
use embassy_usb::{Builder, Config, UsbDevice};
use static_cell::StaticCell;
use usbd_hid::descriptor::{KeyboardReport, MouseReport, SerializedDescriptor};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

pub type UsbDriver = Driver<'static, USB>;
pub type KeyboardWriter = HidWriter<'static, UsbDriver, 8>;
pub type MouseWriter = HidWriter<'static, UsbDriver, 5>;
pub type Device = UsbDevice<'static, UsbDriver>;

struct UsbResources {
    config_descriptor: [u8; 256],
    bos_descriptor: [u8; 256],
    msos_descriptor: [u8; 256],
    control_buffer: [u8; 64],
    keyboard_state: HidState<'static>,
    mouse_state: HidState<'static>,
}

static USB_RESOURCES: StaticCell<UsbResources> = StaticCell::new();

/// Build the USB runner and both writers without unsafe lifetime extension.
pub fn build(usb: embassy_rp::Peri<'static, USB>) -> (Device, KeyboardWriter, MouseWriter) {
    let resources = USB_RESOURCES.init_with(|| UsbResources {
        config_descriptor: [0; 256],
        bos_descriptor: [0; 256],
        msos_descriptor: [0; 256],
        control_buffer: [0; 64],
        keyboard_state: HidState::new(),
        mouse_state: HidState::new(),
    });

    let driver = Driver::new(usb, Irqs);
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Macropad");
    config.product = Some("Macropad HID");
    config.serial_number = Some("00000001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    let mut builder = Builder::new(
        driver,
        config,
        &mut resources.config_descriptor,
        &mut resources.bos_descriptor,
        &mut resources.msos_descriptor,
        &mut resources.control_buffer,
    );

    let keyboard = HidWriter::<_, 8>::new(
        &mut builder,
        &mut resources.keyboard_state,
        HidConfig {
            report_descriptor: KeyboardReport::desc(),
            request_handler: None,
            poll_ms: 10,
            max_packet_size: 8,
            hid_subclass: HidSubclass::No,
            hid_boot_protocol: HidBootProtocol::None,
        },
    );
    let mouse = HidWriter::<_, 5>::new(
        &mut builder,
        &mut resources.mouse_state,
        HidConfig {
            report_descriptor: MouseReport::desc(),
            request_handler: None,
            poll_ms: 10,
            max_packet_size: 5,
            hid_subclass: HidSubclass::No,
            hid_boot_protocol: HidBootProtocol::None,
        },
    );

    (builder.build(), keyboard, mouse)
}
