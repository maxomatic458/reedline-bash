//! Read-only view of the bash history

use reedline::{FileBackedHistory, History, HistoryItem};

/// Builds an in-memory history from bash
///
/// # Safety
/// Calls into bash; must run on the thread bash called into us on.
pub unsafe fn from_bash(window: usize, ignore_prefix: Option<&str>) -> Box<dyn History> {
    let window = window.clamp(1, usize::MAX - 1);
    let mut history = match FileBackedHistory::new(window) {
        Ok(history) => history,
        Err(_) => return Box::<FileBackedHistory>::default(),
    };

    let lines = unsafe { crate::bash::symbols::history_lines() };
    let start = lines.len().saturating_sub(window);
    for line in &lines[start..] {
        if ignore_prefix.is_some_and(|prefix| line.starts_with(prefix)) {
            continue;
        }
        let _ = history.save(HistoryItem::from_command_line(line.as_str()));
    }
    Box::new(history)
}
