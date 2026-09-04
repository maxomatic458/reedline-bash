//! implement reedline's [`Completer`] with bash completion candidates.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use reedline::{Completer, CompletionResult, Span, Suggestion, Suggestions};

use crate::bash::complete::Candidates;
use crate::words;

/// Source of the completion candidates.
pub trait CandidateSource: Send {
    /// The characters that end a word.
    fn word_breaks(&self) -> String;

    fn candidates(&mut self, line: &str, start: usize, pos: usize) -> io::Result<Candidates>;
}

/// Bash itself.
pub struct BashSource;

impl CandidateSource for BashSource {
    fn word_breaks(&self) -> String {
        crate::bash::complete::word_breaks()
    }

    fn candidates(&mut self, line: &str, start: usize, pos: usize) -> io::Result<Candidates> {
        // SAFETY: single-threaded, and the shell is blocked waiting on us.
        Ok(unsafe { crate::bash::complete::candidates(line, start, pos) })
    }
}

/// Counts prompts
pub type PromptCount = Arc<AtomicU64>;

pub struct BashCompleter<S: CandidateSource> {
    source: S,
    /// `true` if the source failed.
    dead: bool,
    /// Which prompt this is.
    prompt_count: PromptCount,
    /// The last answer, reused when the same question is asked again.
    memo: Option<Memo>,
}

/// One remembered completion.
struct Memo {
    /// The prompt the answer was computed at. See [`PromptCount`].
    prompt_count: u64,
    /// The whole line as it was.
    line: String,
    /// The cursor position in that line (byte offset).
    pos: usize,
    /// Completions of the shell.
    suggestions: Suggestions,
}

impl<S: CandidateSource> BashCompleter<S> {
    pub fn new(source: S, prompt_count: PromptCount) -> Self {
        BashCompleter {
            source,
            dead: false,
            prompt_count,
            memo: None,
        }
    }

    fn memoized(&self, line: &str, pos: usize) -> Option<Suggestions> {
        let now = self.prompt_count.load(Ordering::Relaxed);
        self.memo
            .as_ref()
            .filter(|memo| memo.prompt_count == now && memo.line == line && memo.pos == pos)
            .map(|memo| Arc::clone(&memo.suggestions))
    }
}

/// Turn candidates into [`reedline::Suggestion`]s.
fn to_suggestions(candidates: Candidates, line: &str, start: usize, pos: usize) -> Vec<Suggestion> {
    let current_word = &line[start..pos];
    let span = Span::new(start, pos);
    let Candidates {
        matches,
        quote,
        append,
    } = candidates;

    matches
        .into_iter()
        .map(|candidate| {
            let mut value = if quote {
                words::quote_candidate(&candidate, current_word)
            } else {
                candidate.clone()
            };
            // reedline can only add a space, any other character
            // bash needs goes into the word itself.
            let mut whitespace = false;
            match append {
                Some(' ') => whitespace = !candidate.ends_with('/'),
                Some(character) if !value.ends_with(character) => value.push(character),
                _ => {}
            }
            Suggestion {
                value,
                display_override: Some(candidate),
                description: None,
                style: None,
                extra: None,
                span,
                append_whitespace: whitespace,
                match_indices: None,
            }
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
        self.memo = Some(Memo {
            prompt_count: self.prompt_count.load(Ordering::Relaxed),
            line: line.to_string(),
            pos,
            suggestions: Arc::clone(&suggestions),
        });
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
        /// What bash would have reported alongside the matches.
        quote: bool,
        append: Option<char>,
    }

    impl FakeSource {
        fn new(answers: &[&str]) -> Self {
            FakeSource {
                answers: answers.iter().map(|s| s.to_string()).collect(),
                asked: Vec::new(),
                fail: false,
                quote: true,
                append: Some(' '),
            }
        }
    }

    impl CandidateSource for FakeSource {
        fn word_breaks(&self) -> String {
            // stock bash word breaks
            " \t\n\"'><=;|&(:".to_string()
        }

        fn candidates(&mut self, line: &str, _start: usize, pos: usize) -> io::Result<Candidates> {
            self.asked.push((line.to_string(), pos));
            if self.fail {
                return Err(io::Error::other("source failed"));
            }
            Ok(Candidates {
                matches: self.answers.clone(),
                quote: self.quote,
                append: self.append,
            })
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
        let mut completer = BashCompleter::new(
            FakeSource::new(&["checkout", "cherry"]),
            PromptCount::default(),
        );
        let result = completer.complete("git che", 7);
        let spans: Vec<_> = result.suggestions().iter().map(|s| s.span).collect();
        assert!(spans.iter().all(|s| *s == Span::new(4, 7)), "{spans:?}");
        assert_eq!(values(&result), vec!["checkout", "cherry"]);
    }

    #[test]
    fn an_identical_question_is_answered_from_the_memo() {
        let mut completer = BashCompleter::new(FakeSource::new(&["alpha"]), PromptCount::default());
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
    fn a_new_prompt_asks_again_even_for_the_same_question() {
        // `cd` between two prompts, or a file created by the command that ran
        // in between: the same line at the same cursor has a different answer.
        let prompt_count = PromptCount::default();
        let mut completer =
            BashCompleter::new(FakeSource::new(&["alpha"]), Arc::clone(&prompt_count));
        completer.complete("cat a", 5);
        completer.complete("cat a", 5);
        prompt_count.fetch_add(1, Ordering::Relaxed);
        completer.complete("cat a", 5);
        assert_eq!(completer.source.asked.len(), 2);
    }

    #[test]
    fn a_changed_line_or_cursor_asks_again() {
        let mut completer = BashCompleter::new(FakeSource::new(&["alpha"]), PromptCount::default());
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
        let mut completer = BashCompleter::new(source, PromptCount::default());
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
        let mut completer = BashCompleter::new(
            FakeSource::new(&["name with spaces.txt"]),
            PromptCount::default(),
        );
        let result = completer.complete("cat na", 6);
        let suggestion = &result.suggestions()[0];
        assert_eq!(suggestion.value, r"name\ with\ spaces.txt");
        assert_eq!(suggestion.display_value(), "name with spaces.txt");
    }

    #[test]
    fn a_directory_candidate_chains_instead_of_ending_the_word() {
        let mut completer = BashCompleter::new(
            FakeSource::new(&["subdir/", "file.txt"]),
            PromptCount::default(),
        );
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
    fn a_candidate_bash_does_not_want_quoted_is_left_alone() {
        let mut source = FakeSource::new(&["$HISTFILE"]);
        source.quote = false;
        let mut completer = BashCompleter::new(source, PromptCount::default());
        let result = completer.complete("echo $HISTFIL", 13);
        assert_eq!(values(&result), vec!["$HISTFILE"]);
    }

    #[test]
    fn the_character_bash_asks_for_finishes_the_word() {
        // `$HOME` names a directory, so bash asks for a `/` rather than a space.
        let mut source = FakeSource::new(&["$HOME"]);
        source.quote = false;
        source.append = Some('/');
        let mut completer = BashCompleter::new(source, PromptCount::default());
        let result = completer.complete("echo $HOM", 9);
        let suggestion = &result.suggestions()[0];
        assert_eq!(suggestion.value, "$HOME/");
        assert!(!suggestion.append_whitespace, "the slash already ended it");
    }

    #[test]
    fn a_suppressed_append_leaves_the_word_open() {
        let mut source = FakeSource::new(&["alpha"]);
        source.append = None;
        let mut completer = BashCompleter::new(source, PromptCount::default());
        assert!(!completer.complete("git a", 5).suggestions()[0].append_whitespace);
    }

    #[test]
    fn a_cursor_past_the_end_of_the_line_does_not_panic() {
        let mut completer = BashCompleter::new(FakeSource::new(&["alpha"]), PromptCount::default());
        let result = completer.complete("git", 999);
        assert_eq!(result.suggestions()[0].span, Span::new(0, 3));
    }
}
