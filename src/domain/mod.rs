//! Hardware-independent profiles, events, commands, and automaton decisions.

mod automaton;
mod command;
mod display_model;
mod event;
mod profile;

pub use automaton::{AutomatonRuntime, Decision};
pub use command::HidCommand;
pub use display_model::{DisplayModel, DisplayPhase};
pub use event::ControllerEvent;
pub use profile::{
    Direction, HidOutput, Keystrokes, Profile, State, StateType, keycodes, mouse_buttons,
};
