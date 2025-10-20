use crate::config::FractalConfig;

pub struct UndoHistory {
    states: Vec<FractalConfig>,
    current_index: usize,
    max_history: usize,
}

impl UndoHistory {
    pub fn new(initial_state: FractalConfig) -> Self {
        Self {
            states: vec![initial_state],
            current_index: 0,
            max_history: 50,
        }
    }

    /// Push a new state to the history. This clears any redo history.
    pub fn push(&mut self, config: FractalConfig) {
        // Remove any redo states when new action occurs
        self.states.truncate(self.current_index + 1);

        self.states.push(config);

        // Limit history size
        if self.states.len() > self.max_history {
            self.states.remove(0);
        } else {
            self.current_index = self.states.len() - 1;
        }
    }

    /// Undo to the previous state. Returns None if at the beginning of history.
    pub fn undo(&mut self) -> Option<&FractalConfig> {
        if self.current_index > 0 {
            self.current_index -= 1;
            Some(&self.states[self.current_index])
        } else {
            None
        }
    }

    /// Redo to the next state. Returns None if at the end of history.
    pub fn redo(&mut self) -> Option<&FractalConfig> {
        if self.current_index < self.states.len() - 1 {
            self.current_index += 1;
            Some(&self.states[self.current_index])
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        self.current_index > 0
    }

    pub fn can_redo(&self) -> bool {
        self.current_index < self.states.len() - 1
    }
}
