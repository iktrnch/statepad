//! Separate ordered command protocols for output actors.

use super::HidOutput;

/// Commands exclusively consumed by the HID actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HidCommand {
    SetOutput(HidOutput),
    ReleaseAll,
    ReleaseForBootloader { request_id: u32 },
}

/// Commands exclusively consumed by the transition-timer actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerCommand {
    Start { generation: u32, duration_ms: u32 },
    Cancel { generation: u32 },
}
