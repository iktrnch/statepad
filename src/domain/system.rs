//! Fatal runtime failures and user-visible system states.

/// Runtime failures that must be shown before the firmware halts.
///
/// Known fallible operations report one of these instead of panicking directly,
/// because a panic stops the single executor before the OLED actor can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirmwareError {
    UsbWriteTimeout,
    UsbDisabled,
    UsbReportTooLarge,
    HidCommandQueueFull,
    NoProfilesConfigured,
    InvalidProfile,
}

/// Non-application screens rendered by the OLED actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemStatus {
    Fatal(FirmwareError),
    Bootloader,
}
