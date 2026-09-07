//! Transactional undo/redo history for the project.

use crate::schema::Project;

/// Single undoable transaction entry.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub description: String,
    pub project: Project,
}

/// Bounded undo/redo stack for transactional timeline modifications.
#[derive(Debug, Clone)]
pub struct ProjectHistory {
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    max_history: usize,
}

impl Default for ProjectHistory {
    fn default() -> Self {
        Self::new(50)
    }
}

impl ProjectHistory {
    pub fn new(max_history: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: max_history.max(1),
        }
    }

    /// Snapshot the current state before performing an operation.
    pub fn commit(&mut self, current: &Project, description: impl Into<String>) {
        if self.undo_stack.len() >= self.max_history {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(HistoryEntry {
            description: description.into(),
            project: current.clone(),
        });
        self.redo_stack.clear();
    }

    /// Undo the latest action, restoring the previous project state.
    pub fn undo(&mut self, current: &mut Project) -> bool {
        if let Some(entry) = self.undo_stack.pop() {
            let redo_entry = HistoryEntry {
                description: entry.description.clone(),
                project: current.clone(),
            };
            self.redo_stack.push(redo_entry);
            *current = entry.project;
            true
        } else {
            false
        }
    }

    /// Redo an undone action.
    pub fn redo(&mut self, current: &mut Project) -> bool {
        if let Some(entry) = self.redo_stack.pop() {
            let undo_entry = HistoryEntry {
                description: entry.description.clone(),
                project: current.clone(),
            };
            self.undo_stack.push(undo_entry);
            *current = entry.project;
            true
        } else {
            false
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_description(&self) -> Option<&str> {
        self.undo_stack.last().map(|e| e.description.as_str())
    }

    pub fn redo_description(&self) -> Option<&str> {
        self.redo_stack.last().map(|e| e.description.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::CanvasRatio;

    #[test]
    fn test_undo_redo_lifecycle() {
        let mut p = Project::default();
        let mut history = ProjectHistory::new(10);

        assert!(!history.can_undo());
        assert!(!history.can_redo());

        // Step 1: Change canvas ratio
        history.commit(&p, "Change Canvas Ratio");
        p.set_canvas_ratio(CanvasRatio::Vertical9x16);
        assert_eq!(p.settings.width, 1080);
        assert!(history.can_undo());
        assert_eq!(history.undo_description(), Some("Change Canvas Ratio"));

        // Step 2: Undo
        let undone = history.undo(&mut p);
        assert!(undone);
        assert_eq!(p.settings.width, 1920); // restored default 16:9
        assert!(history.can_redo());
        assert_eq!(history.redo_description(), Some("Change Canvas Ratio"));

        // Step 3: Redo
        let redone = history.redo(&mut p);
        assert!(redone);
        assert_eq!(p.settings.width, 1080);
        assert!(!history.can_redo());
    }
}
