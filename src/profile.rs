use usbd_hid::descriptor::KeyboardUsage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateType {
    Left,
    Right,
    Idle,
}

impl From<StateType> for &'static str {
    fn from(value: StateType) -> Self {
        match value {
            StateType::Left => "LEFT",
            StateType::Right => "RIGHT",
            StateType::Idle => "IDLE",
        }
    }
}

/// Modifier bits used by a standard USB keyboard report.
#[allow(unused)]
pub mod modifiers {
    pub const LEFT_CTRL: u8 = 1 << 0;
    pub const LEFT_SHIFT: u8 = 1 << 1;
    pub const LEFT_ALT: u8 = 1 << 2;
    pub const LEFT_GUI: u8 = 1 << 3;

    pub const RIGHT_CTRL: u8 = 1 << 4;
    pub const RIGHT_SHIFT: u8 = 1 << 5;
    pub const RIGHT_ALT: u8 = 1 << 6;
    pub const RIGHT_GUI: u8 = 1 << 7;
}

/// Mouse button bits used by MouseReport.
#[allow(unused)]
pub mod mouse_buttons {
    pub const NONE: u8 = 0;
    pub const LEFT: u8 = 1 << 0;
    pub const RIGHT: u8 = 1 << 1;
    pub const MIDDLE: u8 = 1 << 2;
}

/// Up to six ordinary keys plus the modifier byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keystrokes {
    pub modifiers: u8,
    pub keycodes: [u8; 6],
}

impl Keystrokes {
    pub const NONE: Self = Self {
        modifiers: 0,
        keycodes: [0; 6],
    };

    pub const fn one(key: KeyboardUsage) -> Self {
        Self {
            modifiers: 0,
            keycodes: [key as u8, 0, 0, 0, 0, 0],
        }
    }

    pub const fn two(first: KeyboardUsage, second: KeyboardUsage) -> Self {
        Self {
            modifiers: 0,
            keycodes: [first as u8, second as u8, 0, 0, 0, 0],
        }
    }

    pub const fn three(first: KeyboardUsage, second: KeyboardUsage, third: KeyboardUsage) -> Self {
        Self {
            modifiers: 0,
            keycodes: [first as u8, second as u8, third as u8, 0, 0, 0],
        }
    }

    pub const fn with_modifier(modifier: u8, key: KeyboardUsage) -> Self {
        Self {
            modifiers: modifier,
            keycodes: [key as u8, 0, 0, 0, 0, 0],
        }
    }

    pub const fn from_raw(modifiers: u8, keycodes: [u8; 6]) -> Self {
        Self {
            modifiers,
            keycodes,
        }
    }

    pub const fn report(self) -> [u8; 8] {
        [
            self.modifiers,
            0, // Reserved byte
            self.keycodes[0],
            self.keycodes[1],
            self.keycodes[2],
            self.keycodes[3],
            self.keycodes[4],
            self.keycodes[5],
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HidOutput {
    pub keystrokes: Keystrokes,
    pub mouse_buttons: u8,
}

impl HidOutput {
    pub const NONE: Self = Self {
        keystrokes: Keystrokes::NONE,
        mouse_buttons: mouse_buttons::NONE,
    };

    pub const fn new(keystrokes: Keystrokes, mouse_buttons: u8) -> Self {
        Self {
            keystrokes,
            mouse_buttons,
        }
    }

    pub const fn keyboard_report(self) -> [u8; 8] {
        self.keystrokes.report()
    }

    pub const fn mouse_report(self) -> [u8; 5] {
        [
            self.mouse_buttons,
            0, // X movement
            0, // Y movement
            0, // Wheel
            0, // Horizontal pan
        ]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct State {
    pub state_type: StateType,
    pub output: HidOutput,
}

impl State {
    pub const fn new(state_type: StateType, output: HidOutput) -> Self {
        Self { state_type, output }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Transition {
    pub output: HidOutput,
    pub destination: StateType,
    pub duration_ms: u32,
}

impl Transition {
    pub const fn new(output: HidOutput, destination: StateType, duration_ms: u32) -> Self {
        Self {
            output,
            destination,
            duration_ms,
        }
    }
}

pub struct Profile {
    pub name: &'static str,

    pub left: State,
    pub right: State,
    pub idle: State,

    pub transition_lr: Option<Transition>,
    pub transition_rl: Option<Transition>,
}

/// Result of asking the automaton to move to another stable state.
#[derive(Clone, Copy, Debug)]
pub enum Move {
    /// The automaton is already in the requested state.
    Stay,

    /// Enter the requested stable state immediately.
    Enter(StateType),

    /// Run the supplied transition, then enter its destination.
    Run(Transition),
}

impl Profile {
    pub const fn state(&self, state_type: StateType) -> &State {
        match state_type {
            StateType::Left => &self.left,
            StateType::Right => &self.right,
            StateType::Idle => &self.idle,
        }
    }

    pub fn move_to(&self, current: StateType, target: StateType) -> Move {
        if current == target {
            return Move::Stay;
        }

        match (current, target) {
            // Idle never requires a timed transition.
            (_, StateType::Idle) => Move::Enter(StateType::Idle),

            // Enter either side directly from Idle.
            (StateType::Idle, StateType::Left) => Move::Enter(StateType::Left),
            (StateType::Idle, StateType::Right) => Move::Enter(StateType::Right),

            // Use the configured transition when present.
            (StateType::Left, StateType::Right) => match self.transition_lr {
                Some(transition) => Move::Run(transition),
                None => Move::Enter(StateType::Right),
            },

            (StateType::Right, StateType::Left) => match self.transition_rl {
                Some(transition) => Move::Run(transition),
                None => Move::Enter(StateType::Left),
            },

            // current == target was handled above.
            _ => Move::Stay,
        }
    }
}
