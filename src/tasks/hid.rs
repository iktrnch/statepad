//! Ordered HID output actor owning both report writers.

use core::future::Future;

use embassy_futures::select::{Either, select};
use embassy_sync::channel::{Receiver, Sender};
use embassy_time::{Duration, Timer};
use embassy_usb::driver::EndpointError;

use crate::app::AppRawMutex;
use crate::domain::{ControllerEvent, FirmwareError, HidCommand, HidOutput};
use crate::hardware::usb::{KeyboardWriter, MouseWriter};

type HidReceiver = Receiver<'static, AppRawMutex, HidCommand, 8>;
type EventSender = Sender<'static, AppRawMutex, ControllerEvent, 16>;

const USB_WRITE_TIMEOUT: Duration = Duration::from_secs(1);

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
                if let Err(error) = replace_output(&mut keyboard, &mut mouse, output).await {
                    report_and_drain(error, &commands, events).await;
                }
            }
            HidCommand::ReleaseAll => {
                if let Err(error) = release_all(&mut keyboard, &mut mouse).await {
                    report_and_drain(error, &commands, events).await;
                }
            }
            HidCommand::ReleaseForBootloader { request_id } => {
                // BOOTSEL disconnects the device anyway, so release is best effort.
                let _release_result = release_all(&mut keyboard, &mut mouse).await;
                events
                    .send(ControllerEvent::HidReleasedForBootloader { request_id })
                    .await;
            }
        }
    }
}

async fn replace_output(
    keyboard: &mut KeyboardWriter,
    mouse: &mut MouseWriter,
    output: HidOutput,
) -> Result<(), FirmwareError> {
    release_all(keyboard, mouse).await?;
    write_keyboard(keyboard, &output.keyboard_report()).await?;
    write_mouse(mouse, &output.mouse_report()).await
}

async fn release_all(
    keyboard: &mut KeyboardWriter,
    mouse: &mut MouseWriter,
) -> Result<(), FirmwareError> {
    write_keyboard(keyboard, &[0; 8]).await?;
    write_mouse(mouse, &[0; 5]).await
}

async fn write_keyboard(
    writer: &mut KeyboardWriter,
    report: &[u8; 8],
) -> Result<(), FirmwareError> {
    write_with_timeout(writer.write(report)).await
}

async fn write_mouse(writer: &mut MouseWriter, report: &[u8; 5]) -> Result<(), FirmwareError> {
    write_with_timeout(writer.write(report)).await
}

async fn write_with_timeout(
    write: impl Future<Output = Result<(), EndpointError>>,
) -> Result<(), FirmwareError> {
    match select(write, Timer::after(USB_WRITE_TIMEOUT)).await {
        Either::First(Ok(())) => Ok(()),
        Either::First(Err(EndpointError::Disabled)) => Err(FirmwareError::UsbDisabled),
        Either::First(Err(EndpointError::BufferOverflow)) => Err(FirmwareError::UsbReportTooLarge),
        Either::Second(()) => Err(FirmwareError::UsbWriteTimeout),
    }
}

async fn report_and_drain(error: FirmwareError, commands: &HidReceiver, events: EventSender) -> ! {
    events.send(ControllerEvent::FatalError(error)).await;

    // Keep consuming after failure so no producer can deadlock on a full
    // ordered-command queue while the controller presents the error screen.
    // BOOTSEL remains available if its request won a race with the fatal event.
    loop {
        if let HidCommand::ReleaseForBootloader { request_id } = commands.receive().await {
            events
                .send(ControllerEvent::HidReleasedForBootloader { request_id })
                .await;
        }
    }
}
