#![no_std]
#![no_main]

mod profile;

use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::i2c::{Config as I2cConfig, I2c};
use embassy_rp::peripherals::USB;
use embassy_rp::rom_data::reset_to_usb_boot;
use embassy_rp::usb::{Driver as UsbDriver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel};
use embassy_time::{Duration, Instant, Timer};
use embassy_usb::class::hid::{
    Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State as HidState,
};
use embassy_usb::{Builder as UsbBuilder, Config as UsbConfig};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_7X14},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};
use usbd_hid::descriptor::{KeyboardReport, KeyboardUsage, MouseReport, SerializedDescriptor};

use panic_halt as _;

use profile::{HidOutput, Keystrokes, Move, Profile, State, StateType, Transition, mouse_buttons};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
});

type HidChannel = Channel<NoopRawMutex, HidOutput, 8>;

/// Temporary profile used to exercise the engine.
///
/// Replace this later with your actual profile array.
const TEST_PROFILE: Profile = Profile {
    name: "TEST PROFILE",

    idle: State::new(StateType::Idle, HidOutput::NONE),

    left: State::new(
        StateType::Left,
        HidOutput::new(
            Keystrokes::one(KeyboardUsage::KeyboardAa),
            mouse_buttons::LEFT,
        ),
    ),

    right: State::new(
        StateType::Right,
        HidOutput::new(
            Keystrokes::one(KeyboardUsage::KeyboardSs),
            mouse_buttons::LEFT,
        ),
    ),

    transition_lr: Some(Transition::new(
        HidOutput::new(
            Keystrokes::two(KeyboardUsage::KeyboardWw, KeyboardUsage::KeyboardDd),
            mouse_buttons::NONE,
        ),
        StateType::Right,
        2_000,
    )),

    transition_rl: Some(Transition::new(
        HidOutput::new(
            Keystrokes::two(KeyboardUsage::KeyboardWw, KeyboardUsage::KeyboardAa),
            mouse_buttons::NONE,
        ),
        StateType::Left,
        2_000,
    )),
};

#[derive(Clone, Copy)]
struct ActiveTransition {
    transition: Transition,
    started_at: Instant,
}

/// Queue a complete HID-output replacement.
///
/// An empty report is sent first so keys from the old state cannot
/// remain held when the new state is applied.
async fn publish_output(channel: &HidChannel, output: HidOutput) {
    channel.send(HidOutput::NONE).await;

    if output != HidOutput::NONE {
        channel.send(output).await;
    }
}

/// Ask the profile automaton to move to a stable state.
async fn request_state(
    profile: &Profile,
    current_state: &mut StateType,
    active_transition: &mut Option<ActiveTransition>,
    target: StateType,
    channel: &HidChannel,
) {
    // This first implementation does not allow redirection while a
    // timed transition is running.
    if active_transition.is_some() {
        return;
    }

    match profile.move_to(*current_state, target) {
        Move::Stay => {}

        Move::Enter(destination) => {
            *current_state = destination;

            publish_output(channel, profile.state(destination).output).await;
        }

        Move::Run(transition) => {
            publish_output(channel, transition.output).await;

            *active_transition = Some(ActiveTransition {
                transition,
                started_at: Instant::now(),
            });
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // USB
    let usb_driver = UsbDriver::new(p.USB, Irqs);

    let mut usb_config = UsbConfig::new(0xc0de, 0xcafe);
    usb_config.manufacturer = Some("Macropad");
    usb_config.product = Some("Macropad HID");
    usb_config.serial_number = Some("00000001");
    usb_config.max_power = 100;
    usb_config.max_packet_size_0 = 64;
    usb_config.composite_with_iads = false;
    usb_config.device_class = 0;
    usb_config.device_sub_class = 0;
    usb_config.device_protocol = 0;

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut msos_descriptor = [0; 256];
    let mut control_buffer = [0; 64];

    let mut keyboard_hid_state = HidState::new();
    let mut mouse_hid_state = HidState::new();

    let mut usb_builder = UsbBuilder::new(
        usb_driver,
        usb_config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buffer,
    );

    let keyboard_hid_config = HidConfig {
        report_descriptor: KeyboardReport::desc(),
        request_handler: None,
        poll_ms: 10,
        max_packet_size: 8,
        hid_subclass: HidSubclass::No,
        hid_boot_protocol: HidBootProtocol::None,
    };

    let mouse_hid_config = HidConfig {
        report_descriptor: MouseReport::desc(),
        request_handler: None,
        poll_ms: 10,
        max_packet_size: 5,
        hid_subclass: HidSubclass::No,
        hid_boot_protocol: HidBootProtocol::None,
    };

    let mut keyboard_writer = HidWriter::<_, 8>::new(
        &mut usb_builder,
        &mut keyboard_hid_state,
        keyboard_hid_config,
    );

    let mut mouse_writer =
        HidWriter::<_, 5>::new(&mut usb_builder, &mut mouse_hid_state, mouse_hid_config);

    let mut usb = usb_builder.build();

    // Oled
    let i2c = I2c::new_blocking(
        p.I2C0,
        p.PIN_1, // SCL
        p.PIN_0, // SDA
        I2cConfig::default(),
    );

    let interface = I2CDisplayInterface::new(i2c);

    let mut display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    let _ = display.init();

    /*
     * Physical inputs
     */

    let left_button = Input::new(p.PIN_4, Pull::Up);
    let right_button = Input::new(p.PIN_3, Pull::Up);
    let preset_button = Input::new(p.PIN_2, Pull::Up);

    let hid_channel = HidChannel::new();

    let profile = &TEST_PROFILE;

    let app_fut = async {
        let text_style = MonoTextStyle::new(&FONT_7X14, BinaryColor::On);

        let mut current_state = StateType::Idle;
        let mut active_transition: Option<ActiveTransition> = None;

        let mut previous_pressed = [false; 3];
        let mut last_side_press: [Option<Instant>; 2] = [None; 2];

        let mut preset_held_since: Option<Instant> = None;
        let mut displayed_status: Option<&'static str> = None;

        publish_output(&hid_channel, profile.idle.output).await;

        loop {
            /*
             * Finish a timed transition without blocking the button
             * loop for the entire duration.
             */

            if let Some(active) = active_transition {
                let duration = Duration::from_millis(active.transition.duration_ms as u64);

                if active.started_at.elapsed() >= duration {
                    let destination = active.transition.destination;

                    active_transition = None;
                    current_state = destination;

                    publish_output(&hid_channel, profile.state(destination).output).await;
                }
            }

            let pressed = [
                left_button.is_low(),
                right_button.is_low(),
                preset_button.is_low(),
            ];

            /*
             * Left and right button press events.
             */

            for index in 0..2 {
                if pressed[index] && !previous_pressed[index] {
                    let debounce_complete = match last_side_press[index] {
                        Some(last_press) => last_press.elapsed() >= Duration::from_millis(30),
                        None => true,
                    };

                    if debounce_complete {
                        last_side_press[index] = Some(Instant::now());

                        let target = match index {
                            0 => StateType::Left,
                            1 => StateType::Right,
                            _ => unreachable!(),
                        };

                        request_state(
                            profile,
                            &mut current_state,
                            &mut active_transition,
                            target,
                            &hid_channel,
                        )
                        .await;
                    }
                }
            }

            /*
             * Preset button.
             *
             * Temporary behaviour:
             * - short press: cancel and return to Idle
             * - five-second hold: enter USB BOOTSEL
             */

            if pressed[2] && !previous_pressed[2] {
                preset_held_since = Some(Instant::now());
            }

            if pressed[2] {
                if let Some(held_since) = preset_held_since {
                    if held_since.elapsed() >= Duration::from_secs(5) {
                        publish_output(&hid_channel, HidOutput::NONE).await;

                        // Give the host enough time to receive the
                        // release reports before resetting.
                        Timer::after(Duration::from_millis(30)).await;

                        reset_to_usb_boot(0, 0);
                    }
                }
            } else if previous_pressed[2] {
                if let Some(held_since) = preset_held_since.take() {
                    let held_for = held_since.elapsed();

                    // Ignore very brief contact bounce.
                    if held_for >= Duration::from_millis(30) && held_for < Duration::from_secs(5) {
                        active_transition = None;
                        current_state = StateType::Idle;

                        publish_output(&hid_channel, profile.idle.output).await;
                    }
                }
            }

            previous_pressed = pressed;

            /*
             * OLED state display.
             */

            let status = match active_transition {
                Some(active) => match (current_state, active.transition.destination) {
                    (StateType::Left, StateType::Right) => "LEFT > RIGHT",

                    (StateType::Right, StateType::Left) => "RIGHT > LEFT",

                    _ => "TRANSITION",
                },

                None => current_state.into(),
            };

            if displayed_status != Some(status) {
                display.clear_buffer();

                let _ =
                    Text::with_baseline(profile.name, Point::new(0, 1), text_style, Baseline::Top)
                        .draw(&mut display);

                let _ =
                    Text::with_baseline(status, Point::new(0, 31), text_style, Baseline::Bottom)
                        .draw(&mut display);

                let _ = display.flush();

                displayed_status = Some(status);
            }

            Timer::after(Duration::from_millis(1)).await;
        }
    };

    /*
     * The HID writer receives complete output snapshots in order.
     *
     * This is one task owning both writers, so keyboard and mouse
     * changes are coordinated by the same queue.
     */

    let hid_fut = async {
        loop {
            let output = hid_channel.receive().await;
            let keyboard_report = output.keyboard_report();
            let mouse_report = output.mouse_report();

            let _ = keyboard_writer.write(&keyboard_report).await;

            let _ = mouse_writer.write(&mouse_report).await;
        }
    };

    join(usb.run(), join(app_fut, hid_fut)).await;
}
