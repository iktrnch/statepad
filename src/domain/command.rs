//! Separate ordered command protocols for output actors.

use super::HidOutput;

/// Commands exclusively consumed by the HID actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HidCommand {
    SetOutput(HidOutput),
    ReleaseAll,
    ReleaseForBootloader { request_id: u32 },
}
