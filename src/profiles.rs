use crate::domain::{
    HidOutput, Keystrokes, Profile, State, StateType, Transition, keycodes, mouse_buttons,
};

/// Moonflower, Sunflower, Wild Rose, Sugar Cane
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

/// Nether Wart, Wheat, Potato, Carrot
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
