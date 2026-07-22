//! Events accepted by the controller actor.

/// Physical inputs and actor acknowledgements delivered in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerEvent {
    LeftPressed,
    LeftReleased,
    RightPressed,
    RightReleased,
    NextProfile,
    BootloaderRequested,
    HidReleasedForBootloader { request_id: u32 },
}
