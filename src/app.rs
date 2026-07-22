//! Static application configuration and bounded actor mailboxes.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;

use crate::domain::{ControllerEvent, DisplayModel, HidCommand};

/// Mutex used by every application mailbox.
///
/// All current actors run on one executor, but a critical-section mutex also
/// keeps these primitives safe if a producer later moves into interrupt context.
pub type AppRawMutex = CriticalSectionRawMutex;

pub type ControllerEventChannel = Channel<AppRawMutex, ControllerEvent, 16>;
pub type HidCommandChannel = Channel<AppRawMutex, HidCommand, 8>;
pub type DisplayWatch = Watch<AppRawMutex, DisplayModel, 1>;

/// Ordered, multi-producer application input events.
pub static CONTROLLER_EVENTS: ControllerEventChannel = Channel::new();
/// Ordered controller-to-HID commands.
pub static HID_COMMANDS: HidCommandChannel = Channel::new();
/// Latest-value display state; stale frames may be overwritten.
pub static DISPLAY_MODELS: DisplayWatch = Watch::new();
