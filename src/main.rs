#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Timer};

use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_17, Level::Low);

    loop {
        led.set_high();
        Timer::after(Duration::from_secs(1)).await;

        led.set_low();
        Timer::after(Duration::from_secs(1)).await;
    }
}
