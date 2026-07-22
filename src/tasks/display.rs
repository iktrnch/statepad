//! Latest-value OLED actor and all framebuffer rendering logic.

use embassy_rp::i2c::{Blocking, I2c};
use embassy_rp::peripherals::I2C0;
use embassy_sync::watch;
use embedded_graphics::mono_font::{MonoTextStyle, ascii::FONT_7X14};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

use crate::app::AppRawMutex;
use crate::domain::{Direction, DisplayModel, DisplayPhase, StateType};

type DisplayReceiver = watch::Receiver<'static, AppRawMutex, DisplayModel, 1>;

#[embassy_executor::task]
pub async fn run(i2c: I2c<'static, I2C0, Blocking>, mut models: DisplayReceiver) {
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().expect("SSD1306 initialization failed");

    loop {
        let model = models.changed().await;
        display.clear_buffer();
        render(&mut display, model);
        display.flush().expect("SSD1306 framebuffer flush failed");
    }
}

fn render<DI>(
    display: &mut Ssd1306<
        DI,
        DisplaySize128x32,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x32>,
    >,
    model: DisplayModel,
) where
    DI: WriteOnlyDataCommand,
{
    let style = MonoTextStyle::new(&FONT_7X14, BinaryColor::On);
    draw(
        display,
        model.profile_name,
        Point::new(0, 0),
        style,
        Baseline::Top,
    );
    draw(
        display,
        phase_label(model.phase),
        Point::new(0, 32),
        style,
        Baseline::Bottom,
    );
}

fn draw<DI>(
    display: &mut Ssd1306<
        DI,
        DisplaySize128x32,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x32>,
    >,
    text: &str,
    position: Point,
    style: MonoTextStyle<'_, BinaryColor>,
    baseline: Baseline,
) -> i32
where
    DI: WriteOnlyDataCommand,
{
    Text::with_baseline(text, position, style, baseline)
        .draw(display)
        .expect("SSD1306 framebuffer drawing failed")
        .x
}

fn phase_label(phase: DisplayPhase) -> &'static str {
    match phase {
        DisplayPhase::Idle | DisplayPhase::Stable(StateType::Idle) => "IDLE",
        DisplayPhase::Stable(StateType::Left) => "LEFT",
        DisplayPhase::Stable(StateType::Right) => "RIGHT",
        DisplayPhase::Transitioning(Direction::LeftToRight) => "LEFT > RIGHT",
        DisplayPhase::Transitioning(Direction::RightToLeft) => "RIGHT > LEFT",
    }
}
