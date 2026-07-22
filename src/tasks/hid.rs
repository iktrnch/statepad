//! Ordered HID output actor owning both report writers.

use embassy_sync::channel::{Receiver, Sender};
use embassy_usb::driver::EndpointError;

use crate::app::AppRawMutex;
use crate::domain::{ControllerEvent, HidCommand, HidOutput};
use crate::hardware::usb::{KeyboardWriter, MouseWriter};

type HidReceiver = Receiver<'static, AppRawMutex, HidCommand, 8>;
type EventSender = Sender<'static, AppRawMutex, ControllerEvent, 16>;

#[embassy_executor::task]
pub async fn run(
    mut keyboard: KeyboardWriter,
    mut mouse: MouseWriter,
    commands: HidReceiver,
    events: EventSender,
) {
    loop {
        match commands.receive().await {
            HidCommand::SetOutput(output) => {
                replace_output(&mut keyboard, &mut mouse, output).await;
            }
            HidCommand::ReleaseAll => {
                release_all(&mut keyboard, &mut mouse).await;
            }
            HidCommand::ReleaseForBootloader { request_id } => {
                release_all(&mut keyboard, &mut mouse).await;
                events
                    .send(ControllerEvent::HidReleasedForBootloader { request_id })
                    .await;
            }
        }
    }
}

async fn replace_output(keyboard: &mut KeyboardWriter, mouse: &mut MouseWriter, output: HidOutput) {
    release_all(keyboard, mouse).await;
    write_keyboard(keyboard, &output.keyboard_report()).await;
    write_mouse(mouse, &output.mouse_report()).await;
}

async fn release_all(keyboard: &mut KeyboardWriter, mouse: &mut MouseWriter) {
    write_keyboard(keyboard, &[0; 8]).await;
    write_mouse(mouse, &[0; 5]).await;
}

async fn write_keyboard(writer: &mut KeyboardWriter, report: &[u8; 8]) {
    handle_write_result(writer.write(report).await);
}

async fn write_mouse(writer: &mut MouseWriter, report: &[u8; 5]) {
    handle_write_result(writer.write(report).await);
}

fn handle_write_result(result: Result<(), EndpointError>) {
    match result {
        Ok(()) | Err(EndpointError::Disabled) => {}
        Err(EndpointError::BufferOverflow) => {
            unreachable!("fixed HID reports fit their configured endpoint packets")
        }
    }
}
