//! Reedline's prompt, taken from `PS1` and `PS2`.
//!
//! Both arrive already expanded, so nothing here interprets prompt escapes.
//! The editor draws every row, including a multi-line prompt.

use std::borrow::Cow;

use reedline::{Color, Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus};

/// What `\[` and `\]` expand to. Reedline measures width by stripping ANSI, so
/// these markers would just be printed.
const RL_PROMPT_START_IGNORE: char = '\u{1}';
const RL_PROMPT_END_IGNORE: char = '\u{2}';

pub struct BashPrompt {
    left: String,
    multiline: String,
}

impl BashPrompt {
    pub fn new(ps1: &str, ps2: &str) -> Self {
        // Reedline emits its own colour first; reset so only PS1's apply.
        let left = format!("\u{1b}[0m{}", sanitize(ps1));
        let multiline = sanitize(if ps2.is_empty() { "> " } else { ps2 });
        BashPrompt { left, multiline }
    }
}

/// 1 row, 0 cols
/// This is required because of a bug in reedline (i think, reedline doesnt
/// accept a `""` TODO: verify)
const ZERO_WIDTH_INDICATOR: &str = "\u{1b}[0m";

fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|&c| c != RL_PROMPT_START_IGNORE && c != RL_PROMPT_END_IGNORE)
        .collect()
}

impl Prompt for BashPrompt {
    /// The whole prompt. The indicator can be replaced by the menu.
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.left)
    }

    /// Always empty: native bash has no right prompt.
    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    /// Nothing visible: `PS1` already ends in whatever the user chose. See
    /// [`ZERO_WIDTH_INDICATOR`] for why it is not empty.
    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(ZERO_WIDTH_INDICATOR)
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.multiline)
    }

    /// This is **reedline's** search not **readline**'s.
    fn render_prompt_history_search_indicator(&self, search: PromptHistorySearch) -> Cow<'_, str> {
        let prefix = match search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!(
            "\u{1b}[0m({}reverse-search: {}) ",
            prefix, search.term
        ))
    }

    fn get_prompt_color(&self) -> Color {
        Color::Default
    }

    fn get_indicator_color(&self) -> Color {
        Color::Default
    }

    fn get_prompt_multiline_color(&self) -> Color {
        Color::Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readline_ignore_markers_are_dropped() {
        let ps1 = "\u{1}\u{1b}[32m\u{2}user@host\u{1}\u{1b}[0m\u{2}$ ";
        let prompt = BashPrompt::new(ps1, "> ");
        let rendered = prompt.render_prompt_left().into_owned();
        assert!(!rendered.contains(RL_PROMPT_START_IGNORE));
        assert!(!rendered.contains(RL_PROMPT_END_IGNORE));
        assert!(rendered.contains("\u{1b}[32m"));
        assert!(rendered.ends_with("$ "));
    }

    #[test]
    fn prompt_leads_with_a_reset_so_reedlines_colour_does_not_bleed() {
        let prompt = BashPrompt::new("$ ", "> ");
        assert!(prompt.render_prompt_left().starts_with("\u{1b}[0m"));
    }

    #[test]
    fn a_multiline_ps1_is_rendered_whole_on_the_left() {
        let prompt = BashPrompt::new("user@host ~/dir\n\u{276f} ", "> ");
        let left = prompt.render_prompt_left();
        assert!(left.contains("user@host ~/dir\n"), "{left:?}");
        assert!(left.ends_with("\u{276f} "), "{left:?}");
    }

    #[test]
    fn the_indicator_is_never_empty_and_never_takes_a_column() {
        for ps1 in ["$ ", "one\ntwo\n$ ", "banner\n", "", "\u{276f} "] {
            let prompt = BashPrompt::new(ps1, "> ");
            let indicator = prompt.render_prompt_indicator(PromptEditMode::Emacs);
            assert_eq!(indicator.lines().count(), 1, "{ps1:?} contributes no rows");
            assert_eq!(
                indicator.replace("\u{1b}[0m", ""),
                "",
                "{ps1:?} indicator takes up columns"
            );
        }
    }

    #[test]
    fn a_single_line_ps1_is_untouched() {
        let prompt = BashPrompt::new("RL$ ", "> ");
        assert!(prompt.render_prompt_left().ends_with("RL$ "));
    }

    #[test]
    fn a_ps1_ending_in_a_newline_keeps_its_trailing_row() {
        let prompt = BashPrompt::new("banner\n", "> ");
        assert!(prompt.render_prompt_left().ends_with("banner\n"));
    }

    #[test]
    fn empty_ps2_falls_back_to_something_visible() {
        let prompt = BashPrompt::new("$ ", "");
        assert_eq!(prompt.render_prompt_multiline_indicator(), "> ");
    }
}
