//! Latest-value OLED actor and all framebuffer rendering logic.

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};

use embassy_rp::i2c::{Blocking, I2c};
use embassy_rp::peripherals::I2C0;
use embassy_sync::blocking_mutex::ThreadModeMutex;
use embassy_sync::channel::Sender;
use embassy_sync::watch;
use embedded_graphics::mono_font::{MonoTextStyle, ascii::FONT_7X14};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

use crate::app::AppRawMutex;
use crate::domain::{
    ControllerEvent, Direction, DisplayModel, DisplayPhase, FirmwareError, StateType, SystemStatus,
};

type DisplayReceiver = watch::Receiver<'static, AppRawMutex, DisplayModel, 1>;
type EventSender = Sender<'static, AppRawMutex, ControllerEvent, 16>;
type Oled = Ssd1306<
    I2CInterface<I2c<'static, I2C0, Blocking>>,
    DisplaySize128x32,
    ssd1306::mode::BufferedGraphicsMode<DisplaySize128x32>,
>;

static OLED: ThreadModeMutex<RefCell<Option<Oled>>> = ThreadModeMutex::new(RefCell::new(None));
static PRESERVE_SCREEN_ON_PANIC: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
enum PresentError {
    Draw,
    Flush,
}

#[embassy_executor::task]
pub async fn run(
    i2c: I2c<'static, I2C0, Blocking>,
    mut models: DisplayReceiver,
    events: EventSender,
) {
    let interface = I2CDisplayInterface::new(i2c);
    let display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    let installed = OLED.lock(|cell| {
        let Ok(mut slot) = cell.try_borrow_mut() else {
            return false;
        };
        if slot.is_some() {
            return false;
        }
        *slot = Some(display);
        true
    });
    if !installed {
        panic!("OLED static storage was already initialized");
    }

    if with_oled(|display| display.init().is_ok()) != Some(true) {
        halt_after_oled_error("INIT FAILED");
    }

    loop {
        let model = models.changed().await;
        match with_oled(|display| present(display, model)) {
            Some(Ok(())) => {}
            Some(Err(PresentError::Draw)) => halt_after_oled_error("DRAW FAILED"),
            Some(Err(PresentError::Flush)) => halt_after_oled_error("FLUSH FAILED"),
            None => panic!("OLED static storage is unavailable"),
        }

        if let DisplayModel::System { request_id, .. } = model {
            events
                .send(ControllerEvent::DisplayPresented { request_id })
                .await;
        }
    }
}

fn present(display: &mut Oled, model: DisplayModel) -> Result<(), PresentError> {
    display.clear_buffer();
    if !render(display, model) {
        return Err(PresentError::Draw);
    }
    display.flush().map_err(|_| PresentError::Flush)
}

fn with_oled<R>(f: impl FnOnce(&mut Oled) -> R) -> Option<R> {
    OLED.lock(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        slot.as_mut().map(f)
    })
}

fn halt_after_oled_error(detail: &str) -> ! {
    preserve_screen_on_panic();
    let _attempted = with_oled(|display| attempt_error_frame(display, detail));
    panic!("OLED actor failed after attempting an error frame");
}

/// Keep an already-flushed system error visible during the intentional panic.
pub fn preserve_screen_on_panic() {
    PRESERVE_SCREEN_ON_PANIC.store(true, Ordering::Release);
}

/// Best-effort synchronous screen used after an unexpected task panic.
pub fn show_panic() {
    if PRESERVE_SCREEN_ON_PANIC.load(Ordering::Acquire) {
        return;
    }

    let _attempted = with_oled(|display| {
        display.clear_buffer();
        if render_lines(display, "PANIC", "FIRMWARE HALTED") {
            let _panic_frame_was_flushed = display.flush().is_ok();
        }
    });
}

fn render<DI>(
    display: &mut Ssd1306<
        DI,
        DisplaySize128x32,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x32>,
    >,
    model: DisplayModel,
) -> bool
where
    DI: WriteOnlyDataCommand,
{
    match model {
        DisplayModel::Application {
            profile_name,
            phase,
            output: _,
        } => render_application(display, profile_name, phase),
        DisplayModel::System { status, .. } => render_system(display, status),
    }
}

fn render_application<DI>(
    display: &mut Ssd1306<
        DI,
        DisplaySize128x32,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x32>,
    >,
    profile_name: &str,
    phase: DisplayPhase,
) -> bool
where
    DI: WriteOnlyDataCommand,
{
    render_lines(display, profile_name, phase_label(phase))
}

fn render_system<DI>(
    display: &mut Ssd1306<
        DI,
        DisplaySize128x32,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x32>,
    >,
    status: SystemStatus,
) -> bool
where
    DI: WriteOnlyDataCommand,
{
    let (top, bottom) = match status {
        SystemStatus::Bootloader => ("BOOTLOADER", "FLASHING MODE"),
        SystemStatus::Fatal(FirmwareError::UsbWriteTimeout) => ("USB ERROR", "HOST TIMEOUT"),
        SystemStatus::Fatal(FirmwareError::UsbDisabled) => ("USB ERROR", "DISCONNECTED"),
        SystemStatus::Fatal(FirmwareError::UsbReportTooLarge) => ("SOFTWARE ERROR", "HID REPORT"),
        SystemStatus::Fatal(FirmwareError::HidCommandQueueFull) => {
            ("SOFTWARE ERROR", "HID QUEUE FULL")
        }
        SystemStatus::Fatal(FirmwareError::NoProfilesConfigured) => ("CONFIG ERROR", "NO PROFILES"),
        SystemStatus::Fatal(FirmwareError::InvalidProfile) => ("CONFIG ERROR", "BAD PROFILE"),
    };
    render_lines(display, top, bottom)
}

fn render_lines<DI>(
    display: &mut Ssd1306<
        DI,
        DisplaySize128x32,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x32>,
    >,
    top: &str,
    bottom: &str,
) -> bool
where
    DI: WriteOnlyDataCommand,
{
    let style = MonoTextStyle::new(&FONT_7X14, BinaryColor::On);
    draw(display, top, Point::new(0, 0), style, Baseline::Top).is_some()
        && draw(display, bottom, Point::new(0, 32), style, Baseline::Bottom).is_some()
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
) -> Option<i32>
where
    DI: WriteOnlyDataCommand,
{
    Text::with_baseline(text, position, style, baseline)
        .draw(display)
        .ok()
        .map(|point| point.x)
}

fn attempt_error_frame<DI>(
    display: &mut Ssd1306<
        DI,
        DisplaySize128x32,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x32>,
    >,
    detail: &str,
) where
    DI: WriteOnlyDataCommand,
{
    display.clear_buffer();
    let rendered = render_lines(display, "OLED ERROR", detail);
    if rendered {
        // This is a best-effort second transfer: a broken OLED/I2C path cannot
        // guarantee that it can report its own failure.
        let _error_frame_was_flushed = display.flush().is_ok();
    }
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
