//! Interrupt-backed, debounced physical input actors.

use embassy_futures::select::{Either, select};
use embassy_rp::gpio::Input;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Timer};

use crate::app::AppRawMutex;
use crate::domain::ControllerEvent;

type EventSender = Sender<'static, AppRawMutex, ControllerEvent, 16>;

const DEBOUNCE: Duration = Duration::from_millis(30);
const BOOTLOADER_HOLD: Duration = Duration::from_secs(5);

async fn side_button(
    mut button: Input<'static>,
    events: EventSender,
    pressed: ControllerEvent,
    released: ControllerEvent,
) -> ! {
    loop {
        // RP2040 GPIO edge waits sleep on the GPIO interrupt future; this is not polling.
        button.wait_for_falling_edge().await;
        Timer::after(DEBOUNCE).await;
        if button.is_low() {
            events.send(pressed).await;

            // Require a stably released level before completing a held transition.
            loop {
                button.wait_for_high().await;
                Timer::after(DEBOUNCE).await;
                if button.is_high() {
                    events.send(released).await;
                    break;
                }
            }
        }
    }
}

#[embassy_executor::task]
pub async fn left(button: Input<'static>, events: EventSender) {
    side_button(
        button,
        events,
        ControllerEvent::LeftPressed,
        ControllerEvent::LeftReleased,
    )
    .await;
}

#[embassy_executor::task]
pub async fn right(button: Input<'static>, events: EventSender) {
    side_button(
        button,
        events,
        ControllerEvent::RightPressed,
        ControllerEvent::RightReleased,
    )
    .await;
}

#[embassy_executor::task]
pub async fn preset(mut button: Input<'static>, events: EventSender) {
    loop {
        button.wait_for_falling_edge().await;
        Timer::after(DEBOUNCE).await;
        if !button.is_low() {
            continue;
        }

        match select(button.wait_for_rising_edge(), Timer::after(BOOTLOADER_HOLD)).await {
            Either::First(()) => {
                events.send(ControllerEvent::NextProfile).await;
                Timer::after(DEBOUNCE).await;
            }
            Either::Second(()) if button.is_low() => {
                events.send(ControllerEvent::BootloaderRequested).await;
                // A long press has one classification; its eventual release is consumed here.
                button.wait_for_high().await;
                Timer::after(DEBOUNCE).await;
            }
            Either::Second(()) => {
                // Release raced the five-second deadline but the pin is no longer held.
                events.send(ControllerEvent::NextProfile).await;
                Timer::after(DEBOUNCE).await;
            }
        }
    }
}
