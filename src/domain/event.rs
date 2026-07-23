//! Events accepted by the controller actor.

use super::FirmwareError;

/// Physical inputs and actor acknowledgements delivered in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerEvent {
    LeftPressed,
    LeftReleased,
    RightPressed,
    RightReleased,
    NextProfile,
    BootloaderRequested,
    FatalError(FirmwareError),
    DisplayPresented { request_id: u32 },
    HidReleasedForBootloader { request_id: u32 },
}
