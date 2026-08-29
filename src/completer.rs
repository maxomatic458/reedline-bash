//! implement reedline's [`Completer`] with bash completion candidates.

use std::io;
use std::sync::Arc;

use reedline::{Completer, CompletionResult, Span, Suggestion, Suggestions};

use crate::words;

/// Source of the completion candidates.
pub trait CandidateSource: Send {
    /// The characters that end a word.
    fn word_breaks(&self) -> String;

    fn candidates(&mut self, line: &str, start: usize, pos: usize) -> io::Result<Vec<String>>;
}

/// Bash itself.
pub struct BashSource;

impl CandidateSource for BashSource {
    fn word_breaks(&self) -> String {
        crate::bash::complete::word_breaks()
    }

    fn candidates(&mut self, line: &str, start: usize, pos: usize) -> io::Result<Vec<String>> {
        // SAFETY: single-threaded, and the shell is blocked waiting on us.
        Ok(unsafe { crate::bash::complete::candidates(line, start, pos) })
    }
}

pub struct BashCompleter<S: CandidateSource> {
    source: S,
    /// `true` if the source failed.
    dead: bool,
    /// The last answer + line + cursor it was computed for.
    ///
    /// reedline reasks on menu open, so we memoize the last answer.
    /// e.g opening menu, tab, closing, opening tab again -> uses memo
    memo: Option<(String, usize, Suggestions)>,
}

impl<S: CandidateSource> BashCompleter<S> {
    pub fn new(source: S) -> Self {
        BashCompleter {
            source,
            dead: false,
            memo: None,
        }
    }

    fn memoized(&self, line: &str, pos: usize) -> Option<Suggestions> {
        self.memo
            .as_ref()
            .and_then(|(cached_line, cached_pos, suggestions)| {
                (cached_line == line && *cached_pos == pos).then(|| Arc::clone(suggestions))
            })
    }
}

/// Turn candidates into [`reedline::Suggestion`]s.
fn to_suggestions(
    candidates: Vec<String>,
    line: &str,
    start: usize,
    pos: usize,
) -> Vec<Suggestion> {
    let current_word = &line[start..pos];
    let span = Span::new(start, pos);

    candidates
        .into_iter()
        .map(|candidate| Suggestion {
            value: words::quote_candidate(&candidate, current_word),
            display_override: Some(candidate.clone()),
            description: None,
            style: None,
            extra: None,
            span,
            append_whitespace: !candidate.ends_with('/'),
            match_indices: None,
        })
        .collect()
}

impl<S: CandidateSource> Completer for BashCompleter<S> {
    fn complete(&mut self, line: &str, pos: usize) -> CompletionResult {
        if self.dead {
            return CompletionResult::fresh(Vec::new());
        }
        if let Some(suggestions) = self.memoized(line, pos) {
            return CompletionResult::fresh(suggestions);
        }

        let pos = pos.min(line.len());
        let start = words::word_start_with_breaks(line, pos, &self.source.word_breaks());

        let candidates = match self.source.candidates(line, start, pos) {
            Ok(candidates) => candidates,
            Err(_) => {
                self.dead = true;
                return CompletionResult::fresh(Vec::new());
            }
        };

        let suggestions: Suggestions = to_suggestions(candidates, line, start, pos).into();
        self.memo = Some((line.to_string(), pos, Arc::clone(&suggestions)));
        CompletionResult::fresh(suggestions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSource {
        answers: Vec<String>,
        asked: Vec<(String, usize)>,
        fail: bool,
    }

    impl FakeSource {
        fn new(answers: &[&str]) -> Self {
            FakeSource {
                answers: answers.iter().map(|s| s.to_string()).collect(),
                asked: Vec::new(),
                fail: false,
            }
        }
    }

    impl CandidateSource for FakeSource {
        fn word_breaks(&self) -> String {
            // stock bash word breaks
            " \t\n\"'><=;|&(:".to_string()
        }

        fn candidates(&mut self, line: &str, _start: usize, pos: usize) -> io::Result<Vec<String>> {
            self.asked.push((line.to_string(), pos));
            if self.fail {
                return Err(io::Error::other("source failed"));
            }
            Ok(self.answers.clone())
        }
    }

    fn values(result: &CompletionResult) -> Vec<String> {
        result
            .suggestions()
            .iter()
            .map(|s| s.value.clone())
            .collect()
    }

    #[test]
    fn candidates_replace_the_word_under_the_cursor() {
        let mut completer = BashCompleter::new(FakeSource::new(&["checkout", "cherry"]));
        let result = completer.complete("git che", 7);
        let spans: Vec<_> = result.suggestions().iter().map(|s| s.span).collect();
        assert!(spans.iter().all(|s| *s == Span::new(4, 7)), "{spans:?}");
        assert_eq!(values(&result), vec!["checkout", "cherry"]);
    }

    #[test]
    fn an_identical_question_is_answered_from_the_memo() {
        let mut completer = BashCompleter::new(FakeSource::new(&["alpha"]));
        for _ in 0..5 {
            assert_eq!(values(&completer.complete("git a", 5)), vec!["alpha"]);
        }
        assert_eq!(
            completer.source.asked.len(),
            1,
            "the shell should have been asked once"
        );
    }

    #[test]
    fn a_changed_line_or_cursor_asks_again() {
        let mut completer = BashCompleter::new(FakeSource::new(&["alpha"]));
        completer.complete("git a", 5);
        completer.complete("git a", 4); // same line, cursor moved
        completer.complete("git b", 5); // same cursor, line changed
        completer.complete("git a", 5); // back to the first. Memo cleared by previous
        assert_eq!(
            completer.source.asked,
            vec![
                ("git a".to_string(), 5),
                ("git a".to_string(), 4),
                ("git b".to_string(), 5),
                ("git a".to_string(), 5),
            ]
        );
    }

    #[test]
    fn a_failing_source_is_asked_once_and_then_left_alone() {
        let mut source = FakeSource::new(&["alpha"]);
        source.fail = true;
        let mut completer = BashCompleter::new(source);
        for _ in 0..5 {
            assert!(completer.complete("git a", 5).suggestions().is_empty());
        }
        assert_eq!(
            completer.source.asked.len(),
            1,
            "should stop after the first failure"
        );
    }

    #[test]
    fn candidates_needing_quotes_are_quoted_but_still_display_plainly() {
        let mut completer = BashCompleter::new(FakeSource::new(&["name with spaces.txt"]));
        let result = completer.complete("cat na", 6);
        let suggestion = &result.suggestions()[0];
        assert_eq!(suggestion.value, r"name\ with\ spaces.txt");
        assert_eq!(suggestion.display_value(), "name with spaces.txt");
    }

    #[test]
    fn a_directory_candidate_chains_instead_of_ending_the_word() {
        let mut completer = BashCompleter::new(FakeSource::new(&["subdir/", "file.txt"]));
        let result = completer.complete("cat s", 5);
        assert!(
            !result.suggestions()[0].append_whitespace,
            "directory should chain"
        );
        assert!(
            result.suggestions()[1].append_whitespace,
            "file should end the word"
        );
    }

    #[test]
    fn a_cursor_past_the_end_of_the_line_does_not_panic() {
        let mut completer = BashCompleter::new(FakeSource::new(&["alpha"]));
        let result = completer.complete("git", 999);
        assert_eq!(result.suggestions()[0].span, Span::new(0, 3));
    }
}
