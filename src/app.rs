//! Static application configuration and bounded actor mailboxes.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;

use crate::domain::{
    ControllerEvent, DisplayModel, HidCommand, HidOutput, Keystrokes, Profile, State, StateType,
    TimerCommand, Transition, keycodes, mouse_buttons,
};

/// Mutex used by every application mailbox.
///
/// All current actors run on one executor, but a critical-section mutex also
/// keeps these primitives safe if a producer later moves into interrupt context.
pub type AppRawMutex = CriticalSectionRawMutex;

pub type ControllerEventChannel = Channel<AppRawMutex, ControllerEvent, 16>;
pub type HidCommandChannel = Channel<AppRawMutex, HidCommand, 8>;
pub type TimerCommandChannel = Channel<AppRawMutex, TimerCommand, 4>;
pub type DisplayWatch = Watch<AppRawMutex, DisplayModel, 1>;

/// Ordered, multi-producer application input events.
pub static CONTROLLER_EVENTS: ControllerEventChannel = Channel::new();
/// Ordered controller-to-HID commands.
pub static HID_COMMANDS: HidCommandChannel = Channel::new();
/// Ordered controller-to-timer commands.
pub static TIMER_COMMANDS: TimerCommandChannel = Channel::new();
/// Latest-value display state; stale frames may be overwritten.
pub static DISPLAY_MODELS: DisplayWatch = Watch::new();

const ECLIPSE_ROSE_CANE: Profile = Profile {
    name: "Eclps/Rse/Cane",
    idle: State::new(StateType::Idle, HidOutput::NONE),
    left: State::new(
        StateType::Left,
        HidOutput::new(Keystrokes::one(keycodes::A), mouse_buttons::LEFT),
    ),
    right: State::new(
        StateType::Right,
        HidOutput::new(Keystrokes::one(keycodes::S), mouse_buttons::LEFT),
    ),
    transition_lr: Some(Transition::new(
        HidOutput::new(
            Keystrokes::two(keycodes::W, keycodes::D),
            mouse_buttons::NONE,
        ),
        StateType::Right,
        2_000,
    )),
    transition_rl: Some(Transition::new(
        HidOutput::new(
            Keystrokes::two(keycodes::W, keycodes::A),
            mouse_buttons::NONE,
        ),
        StateType::Left,
        2_000,
    )),
};

const WARTS_AND_CROPS: Profile = Profile {
    name: "Wrts/Crps",
    idle: State::new(StateType::Idle, HidOutput::NONE),
    left: State::new(
        StateType::Left,
        HidOutput::new(Keystrokes::one(keycodes::A), mouse_buttons::LEFT),
    ),
    right: State::new(
        StateType::Right,
        HidOutput::new(Keystrokes::one(keycodes::D), mouse_buttons::LEFT),
    ),
    transition_lr: None,
    transition_rl: None,
};

/// Immutable profile configuration. Mutable runtime state lives only in the controller.
pub static PROFILES: [Profile; 2] = [ECLIPSE_ROSE_CANE, WARTS_AND_CROPS];
