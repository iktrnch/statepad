//! Complete latest-value state published to the OLED actor.

use super::{Direction, HidOutput, StateType};

/// Phase information relevant to rendering, without mutable runtime state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayPhase {
    Idle,
    Stable(StateType),
    Transitioning(Direction),
}

/// Everything the OLED needs to draw one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayModel {
    pub profile_name: &'static str,
    pub phase: DisplayPhase,
    pub output: HidOutput,
}
