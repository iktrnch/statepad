#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pull};

use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_17, Level::Low);
    let mut button = Input::new(p.PIN_3, Pull::Up);

    loop {
        // The button pulls GPIO 3 low while it is held.
        button.wait_for_low().await;
        led.set_high();

        button.wait_for_high().await;
        led.set_low();
    }
}
