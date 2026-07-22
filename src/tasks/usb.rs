//! USB device runner actor.

use crate::hardware::usb::Device;

#[embassy_executor::task]
pub async fn run(mut device: Device) {
    device.run().await;
}
