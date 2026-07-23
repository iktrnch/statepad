//! Dedicated GPIO17 panic indicator.

use core::cell::RefCell;

use embassy_rp::Peri;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::PIN_17;
use embassy_sync::blocking_mutex::CriticalSectionMutex;

static PANIC_LED: CriticalSectionMutex<RefCell<Option<Output<'static>>>> =
    CriticalSectionMutex::new(RefCell::new(None));

/// Configure the onboard LED off and retain ownership for the firmware lifetime.
pub fn init(pin: Peri<'static, PIN_17>) {
    let led = Output::new(pin, Level::Low);
    let installed = PANIC_LED.lock(|cell| {
        let Ok(mut slot) = cell.try_borrow_mut() else {
            return false;
        };
        if slot.is_some() {
            return false;
        }
        *slot = Some(led);
        true
    });
    if !installed {
        panic!("panic LED storage was already initialized");
    }
}

/// Latch the panic LED on without allocation or executor progress.
///
/// The short critical section is independent of the executor and OLED bus, so
/// an unresponsive display cannot prevent this indicator from being set.
pub fn light() {
    PANIC_LED.lock(|cell| {
        if let Ok(mut slot) = cell.try_borrow_mut()
            && let Some(led) = slot.as_mut()
        {
            led.set_high();
        }
    });
}
