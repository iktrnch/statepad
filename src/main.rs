#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::rom_data::reset_to_usb_boot;
use embassy_time::{Duration, Instant, Timer};

use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_17, Level::Low);
    let button_0 = Input::new(p.PIN_2, Pull::Up);
    let button_1 = Input::new(p.PIN_3, Pull::Up);
    let button_2 = Input::new(p.PIN_4, Pull::Up);
    let mut button_0_held_since: Option<Instant> = None;

    loop {
        // Each button pulls its input low while it is held.
        let button_0_is_held = button_0.is_low();

        if button_0_is_held || (button_1.is_low() && button_2.is_low()) {
            led.set_high();
        } else {
            led.set_low();
        }

        if button_0_is_held {
            match button_0_held_since {
                Some(held_since) if held_since.elapsed() >= Duration::from_secs(5) => {
                    reset_to_usb_boot(0, 0);
                }
                Some(_) => {}
                None => button_0_held_since = Some(Instant::now()),
            }
        } else {
            button_0_held_since = None;
        }

        Timer::after(Duration::from_millis(1)).await;
    }
}
