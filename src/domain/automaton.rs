//! Synchronous, allocation-free automaton validation and decisions.

use super::{
    ControllerEvent, Direction, DisplayModel, DisplayPhase, HidCommand, HidOutput, Profile, State,
    StateType,
};

/// Mutable runtime phase, owned only by the controller task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimePhase {
    Idle,
    Stable(StateType),
    Transitioning {
        direction: Direction,
        destination: StateType,
    },
}

/// Fixed-size effects returned by pure automaton operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decision {
    pub hid: Option<HidCommand>,
    pub display: Option<DisplayModel>,
    pub bootloader: bool,
}

/// All mutable application state required to traverse immutable profiles.
pub struct AutomatonRuntime {
    active_profile_index: usize,
    phase: RuntimePhase,
    current_output: HidOutput,
}

impl AutomatonRuntime {
    pub const fn new() -> Self {
        Self {
            active_profile_index: 0,
            phase: RuntimePhase::Idle,
            current_output: HidOutput::NONE,
        }
    }

    pub fn initial_decision(&mut self, profiles: &[Profile]) -> Decision {
        self.enter_idle(&profiles[self.active_profile_index])
    }

    pub fn handle_event(&mut self, event: ControllerEvent, profiles: &[Profile]) -> Decision {
        match event {
            ControllerEvent::LeftPressed => self.request_left(&profiles[self.active_profile_index]),
            ControllerEvent::LeftReleased => {
                self.release_left(&profiles[self.active_profile_index])
            }
            ControllerEvent::RightPressed => {
                self.request_right(&profiles[self.active_profile_index])
            }
            ControllerEvent::RightReleased => {
                self.release_right(&profiles[self.active_profile_index])
            }
            ControllerEvent::NextProfile => self.switch_profile(profiles),
            ControllerEvent::BootloaderRequested => self.request_bootloader(profiles),
            ControllerEvent::FatalError(_)
            | ControllerEvent::DisplayPresented { .. }
            | ControllerEvent::HidReleasedForBootloader { .. } => Decision::default(),
        }
    }

    pub fn request_left(&mut self, profile: &Profile) -> Decision {
        match self.phase {
            RuntimePhase::Idle => self.enter_stable_state(profile, StateType::Left),
            RuntimePhase::Stable(StateType::Right) => match profile.transition_rl {
                Some(transition) => {
                    self.begin_transition(profile, Direction::RightToLeft, transition)
                }
                None => self.enter_stable_state(profile, StateType::Left),
            },
            RuntimePhase::Stable(StateType::Idle) => {
                self.enter_stable_state(profile, StateType::Left)
            }
            RuntimePhase::Stable(StateType::Left) => self.enter_idle(profile),
            RuntimePhase::Transitioning { .. } => Decision::default(),
        }
    }

    pub fn request_right(&mut self, profile: &Profile) -> Decision {
        match self.phase {
            RuntimePhase::Idle => self.enter_stable_state(profile, StateType::Right),
            RuntimePhase::Stable(StateType::Left) => match profile.transition_lr {
                Some(transition) => {
                    self.begin_transition(profile, Direction::LeftToRight, transition)
                }
                None => self.enter_stable_state(profile, StateType::Right),
            },
            RuntimePhase::Stable(StateType::Idle) => {
                self.enter_stable_state(profile, StateType::Right)
            }
            RuntimePhase::Stable(StateType::Right) => self.enter_idle(profile),
            RuntimePhase::Transitioning { .. } => Decision::default(),
        }
    }

    pub fn release_left(&mut self, profile: &Profile) -> Decision {
        self.complete_transition(Direction::RightToLeft, profile)
    }

    pub fn release_right(&mut self, profile: &Profile) -> Decision {
        self.complete_transition(Direction::LeftToRight, profile)
    }

    pub fn enter_idle(&mut self, profile: &Profile) -> Decision {
        debug_assert_eq!(profile.idle.kind, StateType::Idle);
        debug_assert_eq!(profile.idle.output, HidOutput::NONE);
        self.phase = RuntimePhase::Idle;
        self.current_output = HidOutput::NONE;
        Decision {
            hid: Some(HidCommand::ReleaseAll),
            display: Some(self.display_model(profile)),
            bootloader: false,
        }
    }

    pub fn enter_stable_state(&mut self, profile: &Profile, destination: StateType) -> Decision {
        if destination == StateType::Idle {
            return self.enter_idle(profile);
        }

        let state = profile.state(destination);
        debug_assert_eq!(state.kind, destination);
        self.phase = RuntimePhase::Stable(destination);
        self.current_output = state.output;
        Decision {
            hid: Some(HidCommand::SetOutput(state.output)),
            display: Some(self.display_model(profile)),
            bootloader: false,
        }
    }

    pub fn begin_transition(
        &mut self,
        profile: &Profile,
        direction: Direction,
        transition: State,
    ) -> Decision {
        let expected_destination = match direction {
            Direction::LeftToRight => StateType::Right,
            Direction::RightToLeft => StateType::Left,
        };
        debug_assert_eq!(transition.kind, expected_destination);
        self.phase = RuntimePhase::Transitioning {
            direction,
            destination: transition.kind,
        };
        self.current_output = transition.output;
        Decision {
            hid: Some(HidCommand::SetOutput(transition.output)),
            display: Some(self.display_model(profile)),
            bootloader: false,
        }
    }

    pub fn complete_transition(
        &mut self,
        released_direction: Direction,
        profile: &Profile,
    ) -> Decision {
        match self.phase {
            RuntimePhase::Transitioning {
                direction,
                destination,
            } if direction == released_direction => self.enter_stable_state(profile, destination),
            _ => Decision::default(),
        }
    }

    pub fn switch_profile(&mut self, profiles: &[Profile]) -> Decision {
        self.cancel_transition();
        self.active_profile_index = (self.active_profile_index + 1) % profiles.len();
        self.enter_idle(&profiles[self.active_profile_index])
    }

    pub fn cancel_transition(&mut self) {
        self.phase = RuntimePhase::Idle;
    }

    pub fn request_bootloader(&mut self, profiles: &[Profile]) -> Decision {
        self.cancel_transition();
        let profile = &profiles[self.active_profile_index];
        let mut decision = self.enter_idle(profile);
        decision.bootloader = true;
        decision
    }

    fn display_model(&self, profile: &Profile) -> DisplayModel {
        let phase = match self.phase {
            RuntimePhase::Idle | RuntimePhase::Stable(StateType::Idle) => DisplayPhase::Idle,
            RuntimePhase::Stable(state) => DisplayPhase::Stable(state),
            RuntimePhase::Transitioning { direction, .. } => DisplayPhase::Transitioning(direction),
        };
        DisplayModel::Application {
            profile_name: profile.name,
            phase,
            output: self.current_output,
        }
    }
}
