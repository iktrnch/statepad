#![no_std]
#![no_main]

mod app;
mod domain;
mod hardware;
mod profiles;
mod tasks;

use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::i2c::{Config as I2cConfig, I2c};

use panic_halt as _;

use app::{CONTROLLER_EVENTS, DISPLAY_MODELS, HID_COMMANDS, TIMER_COMMANDS};
use profiles::PROFILES;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = embassy_rp::init(Default::default());

    // Init buttons
    let preset_button = Input::new(peripherals.PIN_2, Pull::Up);
    let left_button = Input::new(peripherals.PIN_3, Pull::Up);
    let right_button = Input::new(peripherals.PIN_4, Pull::Up);

    // Init i2c
    let i2c = I2c::new_blocking(
        peripherals.I2C0,
        peripherals.PIN_1,
        peripherals.PIN_0,
        I2cConfig::default(),
    );

    let (usb_device, keyboard_writer, mouse_writer) = hardware::usb::build(peripherals.USB);
    let display_receiver = DISPLAY_MODELS
        .receiver()
        .expect("the display watch has exactly one receiver");

    spawner.spawn(tasks::usb::run(usb_device).expect("USB task pool exhausted"));
    // HID device task
    spawner.spawn(
        tasks::hid::run(
            keyboard_writer,
            mouse_writer,
            HID_COMMANDS.receiver(),
            CONTROLLER_EVENTS.sender(),
        )
        .expect("HID task pool exhausted"),
    );
    // Display task
    spawner.spawn(tasks::display::run(i2c, display_receiver).expect("display task pool exhausted"));
    // Timer task
    spawner.spawn(
        tasks::timer::run(TIMER_COMMANDS.receiver(), CONTROLLER_EVENTS.sender())
            .expect("timer task pool exhausted"),
    );
    // Controller task
    spawner.spawn(
        tasks::controller::run(
            CONTROLLER_EVENTS.receiver(),
            HID_COMMANDS.sender(),
            DISPLAY_MODELS.sender(),
            TIMER_COMMANDS.sender(),
            &PROFILES,
        )
        .expect("controller task pool exhausted"),
    );
    // Button tasks
    spawner.spawn(
        tasks::buttons::left(left_button, CONTROLLER_EVENTS.sender())
            .expect("left-button task pool exhausted"),
    );
    spawner.spawn(
        tasks::buttons::right(right_button, CONTROLLER_EVENTS.sender())
            .expect("right-button task pool exhausted"),
    );
    spawner.spawn(
        tasks::buttons::preset(preset_button, CONTROLLER_EVENTS.sender())
            .expect("preset-button task pool exhausted"),
    );
}
