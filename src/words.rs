//! Splitting a line into the word and the command around the cursor.
//!
//! A candidate replaces from the start of the word up to the cursor, so only
//! the left edge matters. That is the [`Span`](reedline::Span) reedline requires.

use crate::grammar::quoting::{QuoteType, find_quote_type, quoting_function_rust};

/// Byte offset of the word containing `point`.
///
/// `breaks` is `$COMP_WORDBREAKS`.
pub fn word_start_with_breaks(line: &str, point: usize, breaks: &str) -> usize {
    let prefix = &line[..point.min(line.len())];
    let bytes = prefix.as_bytes();

    let mut start = 0usize;
    let mut i = 0usize;
    let mut quote: Option<u8> = None;
    let mut in_word = false;
    let is_break = |c: u8| c != b'"' && c != b'\'' && breaks.as_bytes().contains(&c);

    while i < bytes.len() {
        let c = bytes[i];
        match quote {
            Some(q) => {
                // Inside double quotes a backslash still escapes.
                if q == b'"' && c == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if c == q {
                    quote = None;
                }
                i += 1;
            }
            None => match c {
                b'\\' => {
                    if !in_word {
                        start = i;
                        in_word = true;
                    }
                    i += 2; // escape swallows next byte
                }
                b'\'' | b'"' => {
                    if !in_word {
                        start = i;
                        in_word = true;
                    }
                    quote = Some(c);
                    i += 1;
                }
                _ if is_break(c) => {
                    in_word = false;
                    i += 1;
                }
                _ => {
                    if !in_word {
                        start = i;
                        in_word = true;
                    }
                    i += 1;
                }
            },
        }
    }

    if in_word {
        start
    } else {
        // Cursor sits on a separator
        prefix.len()
    }
}

/// The single command around `point`. `ls; chmod --` gives `chmod --`.
pub fn command_bounds(line: &str, point: usize) -> (usize, usize) {
    let point = point.min(line.len());
    let bytes = line.as_bytes();

    let mut start: usize = 0;
    let mut end = line.len();
    let mut quote: Option<u8> = None;
    let mut i = 0usize;

    while i < bytes.len() {
        let c = bytes[i];
        match quote {
            Some(q) => {
                if q == b'"' && c == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if c == q {
                    quote = None;
                }
                i += 1;
            }
            None => match c {
                b'\\' => i += 2,
                b'\'' | b'"' => {
                    quote = Some(c);
                    i += 1;
                }
                b';' | b'|' | b'&' | b'\n' | b'(' | b'{' => {
                    if i < point {
                        start = i + 1;
                    } else {
                        end = i;
                        break;
                    }
                    i += 1;
                }
                _ => i += 1,
            },
        }
    }

    // Leading whitespace shifts every offset derived from `start`.
    while start < point && matches!(bytes.get(start), Some(b' ') | Some(b'\t')) {
        start += 1;
    }

    (start, end.max(point))
}

/// Quote `candidate` so the shell reads it as one word.
pub fn quote_candidate(candidate: &str, current_word: &str) -> String {
    let style = find_quote_type(current_word).unwrap_or(QuoteType::Backslash);
    quoting_function_rust(candidate, style, true, !candidate.ends_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_bar(spec: &str) -> (String, usize) {
        let point = spec.find('|').expect("test spec needs a | cursor marker");
        (spec.replace('|', ""), point)
    }

    #[test]
    fn plain_words_split_on_whitespace() {
        for (spec, expected) in [
            ("|", 0),
            ("git|", 0),
            ("git |", 4),
            ("git st|", 4),
            ("git  st|", 5),
            ("git status |", 11),
        ] {
            let (line, point) = at_bar(spec);
            assert_eq!(
                word_start_with_breaks(&line, point, BREAKS),
                expected,
                "{spec}"
            );
        }
    }

    #[test]
    fn quotes_and_escapes_hold_a_word_together() {
        for (spec, expected) in [
            (r#"ls "foo ba|"#, 3),
            (r"ls foo\ ba|", 3),
            (r#"ls 'foo ba|"#, 3),
            (r#"ls "a b" c|"#, 9),
            (r#"echo "a\" b|"#, 5),
        ] {
            let (line, point) = at_bar(spec);
            assert_eq!(
                word_start_with_breaks(&line, point, BREAKS),
                expected,
                "{spec}"
            );
        }
    }

    /// `$COMP_WORDBREAKS` on stock bash.
    const BREAKS: &str = " \t\n\"'><=;|&(:";

    #[test]
    fn a_completion_word_breaks_where_readline_breaks() {
        // Verified against real readline: `foo --color=au` -> `au`.
        for (line, point, expected) in [
            ("foo --color=au", 14, 12),
            ("foo host:/pa", 12, 9),
            ("foo a=b", 7, 6),
            ("foo a>b", 7, 6),
            ("foo a|b", 7, 6),
            ("foo plain", 9, 4),
        ] {
            assert_eq!(
                word_start_with_breaks(line, point, BREAKS),
                expected,
                "{line:?}"
            );
        }
    }

    #[test]
    fn a_break_character_inside_quotes_does_not_split_the_word() {
        // The word starts at the quote, so it can be requoted the same way.
        for (line, point, expected) in [
            (r#"cat "a=b"#, 8, 4),
            (r#"cat 'my fi"#, 10, 4),
            (r"cat a\=b", 8, 4),
        ] {
            assert_eq!(
                word_start_with_breaks(line, point, BREAKS),
                expected,
                "{line:?}"
            );
        }
    }

    #[test]
    fn a_simple_line_is_one_command() {
        assert_eq!(command_bounds("chmod --", 8), (0, 8));
        assert_eq!(command_bounds("", 0), (0, 0));
    }

    #[test]
    fn a_command_starts_after_the_separator_before_the_cursor() {
        for (line, point, expected) in [
            ("ls; chmod -", 11, (4, 11)),
            ("ls | grep f", 11, (5, 11)),
            ("a && b", 6, (5, 6)),
            ("( cd /tmp", 9, (2, 9)),
        ] {
            assert_eq!(command_bounds(line, point), expected, "{line:?}");
        }
    }

    #[test]
    fn a_command_ends_at_the_next_separator() {
        let (line, point) = at_bar("chmod -|; ls");
        assert_eq!(command_bounds(&line, point), (0, 7));
    }

    #[test]
    fn a_separator_inside_quotes_does_not_split_the_command() {
        let (line, point) = at_bar(r#"echo "a; b" c|"#);
        assert_eq!(command_bounds(&line, point), (0, 13));
    }

    #[test]
    fn candidates_are_quoted_the_way_the_word_was_opened() {
        assert_eq!(quote_candidate("plain.txt", ""), "plain.txt");
        assert_eq!(quote_candidate("two words", ""), r"two\ words");
        assert_eq!(quote_candidate("two words", "\"tw"), "\"two words\"");
        assert_eq!(quote_candidate("two words", "'tw"), "'two words'");
        assert_eq!(quote_candidate("it's", "'i"), r"'it'\''s'");
    }

    #[test]
    fn a_directory_leaves_the_quote_open_to_be_completed_into() {
        assert_eq!(quote_candidate("my dir/", "\"my "), "\"my dir/");
        assert_eq!(quote_candidate("my file", "\"my "), "\"my file\"");
    }
}
