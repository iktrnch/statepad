//! Complete latest-value state published to the OLED actor.

use super::{Direction, HidOutput, StateType, SystemStatus};

/// Phase information relevant to rendering, without mutable runtime state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayPhase {
    Idle,
    Stable(StateType),
    Transitioning(Direction),
}

/// Everything the OLED needs to draw one complete frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayModel {
    Application {
        profile_name: &'static str,
        phase: DisplayPhase,
        output: HidOutput,
    },
    System {
        request_id: u32,
        status: SystemStatus,
    },
}
