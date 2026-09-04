//! implement reedline's [`Completer`] with bash completion candidates.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use reedline::{Completer, CompletionResult, Span, Suggestion, Suggestions};

use crate::bash::complete::Candidates;
use crate::describe::Describer;
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
    /// Source of a candidate's description.
    describer: Box<dyn Describer>,
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
    pub fn new(source: S, describer: Box<dyn Describer>, prompt_count: PromptCount) -> Self {
        BashCompleter {
            source,
            describer,
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
fn to_suggestions(
    candidates: Candidates,
    line: &str,
    start: usize,
    pos: usize,
    describer: &mut dyn Describer,
) -> Vec<Suggestion> {
    let current_word = &line[start..pos];
    let span = Span::new(start, pos);
    let Candidates {
        matches,
        quote,
        append,
        finished,
        command_names,
        command_words,
    } = candidates;

    matches
        .into_iter()
        .map(|candidate| {
            // Commands, subcommands and options have a man page
            let description = if command_names {
                describer.command(&candidate)
            } else if candidate.starts_with('-') {
                describer.option(&command_words, &candidate)
            } else {
                describer.subcommand(&command_words, &candidate)
            };
            let mut value = if quote {
                words::quote_candidate(&candidate, current_word)
            } else {
                candidate.clone()
            };
            // reedline can only add a space, any other character
            // bash needs goes into the word itself.
            // A word bash-completion finished with a space gets it back.
            let mut whitespace = finished.contains(&candidate);
            match append {
                Some(' ') => whitespace |= !candidate.ends_with('/'),
                Some(character) if !value.ends_with(character) => value.push(character),
                _ => {}
            }
            Suggestion {
                value,
                display_override: Some(candidate),
                description,
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

        let suggestions: Suggestions =
            to_suggestions(candidates, line, start, pos, self.describer.as_mut()).into();
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
    use crate::describe::NoDescriptions;

    struct FakeDescriber(Vec<String>);

    impl Describer for FakeDescriber {
        fn command(&mut self, name: &str) -> Option<String> {
            self.0.push(format!("command {name}"));
            Some(format!("{name} does things"))
        }

        fn subcommand(&mut self, command: &[String], name: &str) -> Option<String> {
            self.0
                .push(format!("subcommand {} {name}", command.join(" ")));
            (name == "add").then(|| format!("{name} under {}", command.join(" ")))
        }

        fn option(&mut self, command: &[String], option: &str) -> Option<String> {
            self.0
                .push(format!("option {} {option}", command.join(" ")));
            Some(format!("{option} of {}", command.join(" ")))
        }
    }

    struct FakeSource {
        answers: Vec<String>,
        asked: Vec<(String, usize)>,
        fail: bool,
        /// What bash would have reported alongside the matches.
        quote: bool,
        append: Option<char>,
        finished: Vec<&'static str>,
        command_names: bool,
    }

    impl FakeSource {
        fn new(answers: &[&str]) -> Self {
            FakeSource {
                answers: answers.iter().map(|s| s.to_string()).collect(),
                asked: Vec::new(),
                fail: false,
                quote: true,
                append: Some(' '),
                finished: Vec::new(),
                command_names: false,
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
                finished: self.finished.iter().map(|s| s.to_string()).collect(),
                command_names: self.command_names,
                command_words: line[..pos.min(line.len())]
                    .split_whitespace()
                    .take_while(|word| {
                        !line[..pos.min(line.len())].ends_with(word)
                            || line[..pos.min(line.len())].ends_with(' ')
                    })
                    .map(str::to_string)
                    .collect(),
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
            Box::new(NoDescriptions),
            PromptCount::default(),
        );
        let result = completer.complete("git che", 7);
        let spans: Vec<_> = result.suggestions().iter().map(|s| s.span).collect();
        assert!(spans.iter().all(|s| *s == Span::new(4, 7)), "{spans:?}");
        assert_eq!(values(&result), vec!["checkout", "cherry"]);
    }

    #[test]
    fn an_identical_question_is_answered_from_the_memo() {
        let mut completer = BashCompleter::new(
            FakeSource::new(&["alpha"]),
            Box::new(NoDescriptions),
            PromptCount::default(),
        );
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
        let mut completer = BashCompleter::new(
            FakeSource::new(&["alpha"]),
            Box::new(NoDescriptions),
            Arc::clone(&prompt_count),
        );
        completer.complete("cat a", 5);
        completer.complete("cat a", 5);
        prompt_count.fetch_add(1, Ordering::Relaxed);
        completer.complete("cat a", 5);
        assert_eq!(completer.source.asked.len(), 2);
    }

    #[test]
    fn a_changed_line_or_cursor_asks_again() {
        let mut completer = BashCompleter::new(
            FakeSource::new(&["alpha"]),
            Box::new(NoDescriptions),
            PromptCount::default(),
        );
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
        let mut completer =
            BashCompleter::new(source, Box::new(NoDescriptions), PromptCount::default());
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
            Box::new(NoDescriptions),
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
            Box::new(NoDescriptions),
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
        let mut completer =
            BashCompleter::new(source, Box::new(NoDescriptions), PromptCount::default());
        let result = completer.complete("echo $HISTFIL", 13);
        assert_eq!(values(&result), vec!["$HISTFILE"]);
    }

    #[test]
    fn the_character_bash_asks_for_finishes_the_word() {
        // `$HOME` names a directory, so bash asks for a `/` rather than a space.
        let mut source = FakeSource::new(&["$HOME"]);
        source.quote = false;
        source.append = Some('/');
        let mut completer =
            BashCompleter::new(source, Box::new(NoDescriptions), PromptCount::default());
        let result = completer.complete("echo $HOM", 9);
        let suggestion = &result.suggestions()[0];
        assert_eq!(suggestion.value, "$HOME/");
        assert!(!suggestion.append_whitespace, "the slash already ended it");
    }

    #[test]
    fn a_word_bash_completion_finished_gets_its_space_back() {
        // git's completion answers `add ` under `-o nospace`: no append
        // character, but the word itself carries the space readline inserts.
        let mut source = FakeSource::new(&["add", "am"]);
        source.append = None;
        source.finished = vec!["add"];
        let mut completer =
            BashCompleter::new(source, Box::new(NoDescriptions), PromptCount::default());
        let result = completer.complete("git a", 5);
        assert!(
            result.suggestions()[0].append_whitespace,
            "add was finished"
        );
        assert!(!result.suggestions()[1].append_whitespace, "am was not");
    }

    #[test]
    fn a_suppressed_append_leaves_the_word_open() {
        let mut source = FakeSource::new(&["alpha"]);
        source.append = None;
        let mut completer =
            BashCompleter::new(source, Box::new(NoDescriptions), PromptCount::default());
        assert!(!completer.complete("git a", 5).suggestions()[0].append_whitespace);
    }

    #[test]
    fn a_cursor_past_the_end_of_the_line_does_not_panic() {
        let mut completer = BashCompleter::new(
            FakeSource::new(&["alpha"]),
            Box::new(NoDescriptions),
            PromptCount::default(),
        );
        let result = completer.complete("git", 999);
        assert_eq!(result.suggestions()[0].span, Span::new(0, 3));
    }

    #[test]
    fn a_command_name_is_described_as_a_command() {
        let mut source = FakeSource::new(&["ping", "pinky"]);
        source.command_names = true;
        let mut completer = BashCompleter::new(
            source,
            Box::new(FakeDescriber(Vec::new())),
            PromptCount::default(),
        );
        let result = completer.complete("pin", 3);
        let described: Vec<_> = result
            .suggestions()
            .iter()
            .map(|s| s.description.clone())
            .collect();
        assert_eq!(
            described,
            vec![
                Some("ping does things".into()),
                Some("pinky does things".into())
            ]
        );
    }

    #[test]
    fn a_subcommand_is_described_and_a_file_is_not() {
        let source = FakeSource::new(&["add", "notes.txt"]);
        let mut completer = BashCompleter::new(
            source,
            Box::new(FakeDescriber(Vec::new())),
            PromptCount::default(),
        );
        let result = completer.complete("git a", 5);
        let suggestions = result.suggestions();
        assert_eq!(suggestions[0].description.as_deref(), Some("add under git"));
        assert_eq!(suggestions[1].description, None);
    }

    #[test]
    fn an_option_is_described_for_the_subcommand_it_follows() {
        let source = FakeSource::new(&["--verbose"]);
        let mut completer = BashCompleter::new(
            source,
            Box::new(FakeDescriber(Vec::new())),
            PromptCount::default(),
        );
        let result = completer.complete("git add --v", 11);
        assert_eq!(
            result.suggestions()[0].description.as_deref(),
            Some("--verbose of git add")
        );
    }

    #[test]
    fn an_option_is_described_for_its_command_and_a_file_is_not() {
        let source = FakeSource::new(&["--count", "notes.txt"]);
        let mut completer = BashCompleter::new(
            source,
            Box::new(FakeDescriber(Vec::new())),
            PromptCount::default(),
        );
        let result = completer.complete("ping -", 6);
        let suggestions = result.suggestions();
        assert_eq!(
            suggestions[0].description.as_deref(),
            Some("--count of ping")
        );
        assert_eq!(suggestions[1].description, None, "a file has no manual");
    }
}
