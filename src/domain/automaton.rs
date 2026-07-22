//! Synchronous, allocation-free automaton validation and decisions.

use super::{
    ControllerEvent, Direction, DisplayModel, DisplayPhase, HidCommand, HidOutput, Profile,
    StateType, TimerCommand, Transition,
};

/// Mutable runtime phase, owned only by the controller task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimePhase {
    Idle,
    Stable(StateType),
    Transitioning {
        direction: Direction,
        generation: u32,
        destination: StateType,
    },
}

/// Fixed-size effects returned by pure automaton operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decision {
    pub hid: Option<HidCommand>,
    pub display: Option<DisplayModel>,
    pub timer: Option<TimerCommand>,
    pub bootloader: bool,
}

/// All mutable application state required to traverse immutable profiles.
pub struct AutomatonRuntime {
    active_profile_index: usize,
    phase: RuntimePhase,
    next_transition_generation: u32,
    current_output: HidOutput,
}

impl AutomatonRuntime {
    pub const fn new() -> Self {
        Self {
            active_profile_index: 0,
            phase: RuntimePhase::Idle,
            next_transition_generation: 1,
            current_output: HidOutput::NONE,
        }
    }

    pub fn initial_decision(&mut self, profiles: &[Profile]) -> Decision {
        self.enter_idle(&profiles[self.active_profile_index], None)
    }

    pub fn handle_event(&mut self, event: ControllerEvent, profiles: &[Profile]) -> Decision {
        match event {
            ControllerEvent::LeftPressed => self.request_left(&profiles[self.active_profile_index]),
            ControllerEvent::RightPressed => {
                self.request_right(&profiles[self.active_profile_index])
            }
            ControllerEvent::NextProfile => self.switch_profile(profiles),
            ControllerEvent::BootloaderRequested => self.request_bootloader(profiles),
            ControllerEvent::TransitionElapsed { generation } => {
                self.complete_transition(generation, &profiles[self.active_profile_index])
            }
            ControllerEvent::HidReleasedForBootloader { .. } => Decision::default(),
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
            RuntimePhase::Stable(StateType::Left) | RuntimePhase::Transitioning { .. } => {
                self.enter_idle(profile, None)
            }
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
            RuntimePhase::Stable(StateType::Right) | RuntimePhase::Transitioning { .. } => {
                self.enter_idle(profile, None)
            }
        }
    }

    pub fn enter_idle(&mut self, profile: &Profile, timer: Option<TimerCommand>) -> Decision {
        debug_assert_eq!(profile.idle.kind, StateType::Idle);
        debug_assert_eq!(profile.idle.output, HidOutput::NONE);
        self.phase = RuntimePhase::Idle;
        self.current_output = HidOutput::NONE;
        Decision {
            hid: Some(HidCommand::ReleaseAll),
            display: Some(self.display_model(profile)),
            timer,
            bootloader: false,
        }
    }

    pub fn enter_stable_state(&mut self, profile: &Profile, destination: StateType) -> Decision {
        if destination == StateType::Idle {
            return self.enter_idle(profile, None);
        }

        let state = profile.state(destination);
        debug_assert_eq!(state.kind, destination);
        self.phase = RuntimePhase::Stable(destination);
        self.current_output = state.output;
        Decision {
            hid: Some(HidCommand::SetOutput(state.output)),
            display: Some(self.display_model(profile)),
            timer: None,
            bootloader: false,
        }
    }

    pub fn begin_transition(
        &mut self,
        profile: &Profile,
        direction: Direction,
        transition: Transition,
    ) -> Decision {
        let generation = self.allocate_generation();
        self.phase = RuntimePhase::Transitioning {
            direction,
            generation,
            destination: transition.destination,
        };
        self.current_output = transition.output;
        Decision {
            hid: Some(HidCommand::SetOutput(transition.output)),
            display: Some(self.display_model(profile)),
            timer: Some(TimerCommand::Start {
                generation,
                duration_ms: transition.duration_ms,
            }),
            bootloader: false,
        }
    }

    pub fn complete_transition(&mut self, generation: u32, profile: &Profile) -> Decision {
        match self.phase {
            RuntimePhase::Transitioning {
                generation: active_generation,
                destination,
                ..
            } if generation == active_generation => self.enter_stable_state(profile, destination),
            _ => Decision::default(),
        }
    }

    pub fn switch_profile(&mut self, profiles: &[Profile]) -> Decision {
        let timer = self.cancel_transition();
        self.active_profile_index = (self.active_profile_index + 1) % profiles.len();
        self.enter_idle(&profiles[self.active_profile_index], timer)
    }

    pub fn cancel_transition(&mut self) -> Option<TimerCommand> {
        let command = match self.phase {
            RuntimePhase::Transitioning { generation, .. } => {
                Some(TimerCommand::Cancel { generation })
            }
            _ => None,
        };
        self.phase = RuntimePhase::Idle;
        command
    }

    pub fn request_bootloader(&mut self, profiles: &[Profile]) -> Decision {
        let timer = self.cancel_transition();
        let profile = &profiles[self.active_profile_index];
        let mut decision = self.enter_idle(profile, timer);
        decision.bootloader = true;
        decision
    }

    fn allocate_generation(&mut self) -> u32 {
        let generation = self.next_transition_generation;
        self.next_transition_generation = self.next_transition_generation.wrapping_add(1);
        generation
    }

    fn display_model(&self, profile: &Profile) -> DisplayModel {
        let phase = match self.phase {
            RuntimePhase::Idle | RuntimePhase::Stable(StateType::Idle) => DisplayPhase::Idle,
            RuntimePhase::Stable(state) => DisplayPhase::Stable(state),
            RuntimePhase::Transitioning { direction, .. } => DisplayPhase::Transitioning(direction),
        };
        DisplayModel {
            profile_name: profile.name,
            phase,
            output: self.current_output,
        }
    }
}
