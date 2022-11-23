use std::collections::HashMap;
use std::fmt::{self, Display};
use std::slice::Iter;

use crate::inputs::key::Key;

/// We define all available action
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Action {
    Quit,
    ShowLogs,
    ExecCommands,
    SendCMD,
    Next,
    Previous,
    ScrollUp,
    ScrollDown,
    Search,
    Remove,
    StopContainer,
    PauseContainer,
}

impl Action {
    /// All available actions
    pub fn iterator() -> Iter<'static, Action> {
        static ACTIONS: [Action; 12] = [
            Action::Quit,
            Action::ShowLogs,
            Action::ExecCommands,
            Action::SendCMD,
            Action::Next,
            Action::Previous,
            Action::ScrollUp,
            Action::ScrollDown,
            Action::Search,
            Action::Remove,
            Action::StopContainer,
            Action::PauseContainer,
        ];
        ACTIONS.iter()
    }

    /// List of key associated to action
    pub fn keys(&self) -> &[Key] {
        match self {
            Action::Quit => &[Key::Char('q'), Key::Ctrl('c'), Key::Esc],
            Action::ShowLogs => &[Key::Char('l'), Key::Enter],
            Action::ExecCommands => &[Key::Char('e')],
            Action::SendCMD => &[Key::Enter],
            Action::Next => &[Key::Down, Key::Char('n'), Key::Right],
            Action::Previous => &[Key::Up, Key::Char('p'), Key::Left],
            Action::Search => &[Key::Char('/'), Key::Enter],
            Action::ScrollUp => &[Key::Up],
            Action::ScrollDown => &[Key::Down],
            Action::Remove => &[Key::Backspace],
            Action::StopContainer => &[Key::Char('s')],
            Action::PauseContainer => &[Key::Char('p')],
        }
    }
}

/// Could display a user friendly short description of action
impl Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            Action::Quit => "Quit",
            Action::ShowLogs => "Show Logs",
            Action::ExecCommands => "Exec CMD",
            Action::SendCMD => "Send CMD",
            Action::Next => "Next",
            Action::Previous => "Previous",
            Action::Search => "Search",
            Action::ScrollUp => "Scroll Up",
            Action::ScrollDown => "Scroll Down",
            Action::Remove => "Remove",
            Action::StopContainer => "Stop Container",
            Action::PauseContainer => "Pause Container",
        };
        let key = self.keys().first().unwrap();
        write!(f, "{} {}", key, str)
    }
}

/// The application should have some contextual actions.
#[derive(Default, Debug, Clone)]
pub struct Actions(Vec<Action>);

impl Actions {
    /// Given a key, find the corresponding action
    pub fn find(&self, key: Key) -> Option<&Action> {
        Action::iterator()
            .filter(|action| self.0.contains(action))
            .find(|action| action.keys().contains(&key))
    }

    /// Get contextual actions.
    /// (just for building a help view)
    pub fn actions(&self) -> &[Action] {
        self.0.as_slice()
    }
}

impl From<Vec<Action>> for Actions {
    /// Build contextual action
    ///
    /// # Panics
    ///
    /// If two actions have same key
    fn from(actions: Vec<Action>) -> Self {
        // Check key unicity
        let mut map: HashMap<Key, Vec<Action>> = HashMap::new();
        for action in actions.iter() {
            for key in action.keys().iter() {
                match map.get_mut(key) {
                    Some(vec) => vec.push(*action),
                    None => {
                        map.insert(*key, vec![*action]);
                    }
                }
            }
        }
        let errors = map
            .iter()
            .filter(|(_, actions)| actions.len() > 1) // at least two actions share same shortcut
            .map(|(key, actions)| {
                let actions = actions
                    .iter()
                    .map(Action::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Conflict key {} with actions {}", key, actions)
            })
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            panic!("{}", errors.join("; "))
        }

        // Ok, we can create contextual actions
        Self(actions)
    }
}

impl Display for Actions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let actions = self
            .0
            .iter()
            .map(Action::to_string)
            .collect::<Vec<_>>()
            .join(" | ");
        write!(f, "{}", actions)
    }
}
