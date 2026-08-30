// The quoted C keeps bash's own tab alignment.
#![allow(clippy::tabs_in_doc_comments)]

use super::symbols;
use crate::grammar::quoting::{QuoteType, dequoting_function_rust, find_quote_type};
use std::ffi::{CStr, CString, c_char, c_int, c_ulong};

/// How readline would have finished the word.
pub struct Candidates {
    pub matches: Vec<String>,
    /// Whether a match still has to be quoted. Readlines `QUOTING_DESIRED`
    pub quote: bool,
    /// appended to a lone match.
    pub append: Option<char>,
}

/// Candidates for the word between `word_start` and `point`.
///
/// # Safety
/// Calls into bash.
pub unsafe fn candidates(line: &str, word_start: usize, point: usize) -> Candidates {
    let point = point.min(line.len());
    let word_start = word_start.min(point);
    let word = &line[word_start..point];

    // split `COMP_WORDS` as bash would
    let breaks = word_breaks();

    // Bash needs the command bounds (not word bounds)
    let (mut command_start, command_end) = crate::words::command_bounds(line, point);
    command_start += assignment_prefix(&line[command_start..command_end]);
    let command = line[command_start..command_end]
        .split_whitespace()
        .next()
        .unwrap_or("");

    let (bare_word, quote_char) = dequote(word);

    let (Ok(c_word), Ok(c_command)) = (CString::new(bare_word.as_str()), CString::new(command))
    else {
        // NUL cannot be typed and C strings dont contain it.
        return Candidates {
            matches: Vec::new(),
            quote: false,
            append: None,
        };
    };

    unsafe {
        publish_line(line, point);
        publish_word_breaks(&breaks);

        // Readline resets these before every completion.
        symbols::rl_completion_quote_character = c_int::from(quote_char.unwrap_or(0));
        symbols::rl_completion_found_quote = c_int::from(quote_char.is_some());
        symbols::rl_filename_completion_desired = 0;
        symbols::rl_filename_quoting_desired = 1;
        symbols::rl_completion_suppress_append = 0;
        symbols::rl_completion_append_character = c_int::from(b' ');
        symbols::rl_sort_completion_matches = 1;
        symbols::rl_attempted_completion_over = 0;

        symbols::rl_readline_state |= RL_STATE_COMPLETING;

        let cmdpos = in_command_position(line, word_start);

        let mut found: c_int = 0;
        let mut result = Vec::new();
        if !cmdpos {
            let matches = symbols::programmable_completions(
                c_command.as_ptr(),
                c_word.as_ptr(),
                command_start as c_int,
                command_end as c_int,
                &mut found,
            );

            if found != 0 {
                symbols::pcomp_set_readline_variables(found, 1);
            }

            // compspec's element 0 is a real candidate
            result = take_matches(matches, Convention::Plain);
        }

        // a compspec that ran decides whether the fallbacks may still fire.
        // example:
        //
        //   complete -W 'red green blue' widget    `widget a<TAB>` -> nothing
        //   complete -o default -W '...'  widget   `widget a<TAB>` -> file.txt
        let ran = found != 0;
        let allow_bash_default = !ran || found & COPT_BASHDEFAULT != 0;
        let allow_filenames = !ran || found & COPT_DEFAULT != 0;

        // Command names, aliases, functions, ... supplied by bashs `bash_default_completion`
        if result.is_empty() && allow_bash_default && symbols::rl_attempted_completion_over == 0 {
            let flags = if cmdpos { DEFCOMP_CMDPOS } else { 0 };
            let fallback = symbols::bash_default_completion(
                c_word.as_ptr(),
                word_start as c_int,
                point as c_int,
                0,
                flags,
            );
            result = take_matches(fallback, Convention::CommonPrefixFirst);
        }

        // Plain filenames supplied by readline's `rl_filename_completion_function`
        if result.is_empty() && allow_filenames && symbols::rl_attempted_completion_over == 0 {
            let files = symbols::rl_completion_matches(
                c_word.as_ptr(),
                symbols::rl_filename_completion_function,
            );
            result = take_matches(files, Convention::CommonPrefixFirst);
        }

        // Set when the matches are paths. Command names are not, and marking
        // those would turn `ech` into `echo/` whenever a directory of that name
        // happens to be in the cwd.
        let filenames = symbols::rl_filename_completion_desired != 0;

        let quote = symbols::rl_full_quoting_desired != 0
            || (filenames && symbols::rl_filename_quoting_desired != 0);
        let append = if symbols::rl_completion_suppress_append == 0 {
            u8::try_from(symbols::rl_completion_append_character)
                .ok()
                .filter(|&byte| byte != 0)
                .map(char::from)
        } else {
            None
        };

        symbols::rl_readline_state &= !RL_STATE_COMPLETING;

        // Both of these can turn two different strings into one, so the
        // duplicates they make have to be removed after them, not before.
        strip_trailing_space(&mut result);
        if filenames {
            mark_directories(&mut result);
        }
        dedupe(&mut result);
        Candidates {
            matches: result,
            quote,
            append,
        }
    }
}

/// ```c
/// #define RL_STATE_COMPLETING	0x00004000	/* doing completion */
/// ```
/// [`lib/readline/readline.h:934`]
///
/// [`lib/readline/readline.h:934`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n934
const RL_STATE_COMPLETING: c_ulong = 0x0000_4000;

/// ```c
/// /* Flag values for the final argument to bash_default_completion */
/// #define DEFCOMP_CMDPOS		1
/// ```
/// [`bashline.c:342`] — the word is a command name, so builtins, functions,
/// aliases and `$PATH` are candidates.
///
/// [`bashline.c:342`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/bashline.c?h=bash-5.3#n342
const DEFCOMP_CMDPOS: c_int = 1;

/// ```c
/// #define COPT_DEFAULT	(1<<1)
/// ```
/// [`pcomplete.h:71`] — `complete -o default`: fall back to filename completion
/// when the compspec finds nothing.
///
/// [`pcomplete.h:71`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/pcomplete.h?h=bash-5.3#n71
const COPT_DEFAULT: c_int = 1 << 1;

/// ```c
/// #define COPT_BASHDEFAULT (1<<6)
/// ```
/// [`pcomplete.h:76`] — `complete -o bashdefault`: fall back to bash's own
/// default completion first.
///
/// [`pcomplete.h:76`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/pcomplete.h?h=bash-5.3#n76
const COPT_BASHDEFAULT: c_int = 1 << 6;

/// Drop repeats, keeping the first of each.
///
/// Bash reports a name once per source, so `echo` arrives as builtin,
/// `/usr/bin/echo` and `/bin/echo`. Readline dedupes while printing; we have to
/// do it ourselves.
fn dedupe(candidates: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.clone()));
}

/// Whether the word starting at `word_start` is where a command name goes.
///
/// That is the start of the line, or right after something that ends a command.
fn in_command_position(line: &str, word_start: usize) -> bool {
    let before = line[..word_start].trim_end_matches([' ', '\t']);
    match before.chars().last() {
        None => true,
        Some(c) => matches!(c, ';' | '|' | '&' | '(' | '{' | '!' | '\n'),
    }
}

/// Length of the `FOO=bar BAZ=qux ` prefix at the front of a command.
///
/// The command is the first word after it, and readline hands a completion
/// function a `COMP_LINE` that starts there: `FOO=bar widget a` arrives as
/// `widget a`.
fn assignment_prefix(command: &str) -> usize {
    let mut consumed = 0;
    for word in command.split_inclusive(char::is_whitespace) {
        let Some((name, _)) = word.trim_end().split_once('=') else {
            break;
        };
        // `PATH+=` appends, `arr[0]=` assigns to one element.
        let name = name.strip_suffix('+').unwrap_or(name);
        let name = match name.split_once('[') {
            Some((base, subscript)) if subscript.ends_with(']') => base,
            Some(_) => break,
            None => name,
        };
        let mut chars = name.chars();
        let is_name = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !is_name {
            break;
        }
        consumed += word.len();
    }
    consumed
}

/// Put the line where completion functions look for it.
///
/// Bash builds `COMP_LINE` from `rl_line_buffer`. Readline is not running, so
/// nothing else maintains it.
///
/// # Safety
/// Calls bash's allocator. Must run on bash's thread.
unsafe fn publish_line(line: &str, point: usize) {
    unsafe {
        // Readline reallocates this in place, so it has to be bash's to free.
        let previous = symbols::rl_line_buffer;
        symbols::rl_line_buffer = symbols::bash_strdup(line);
        symbols::rl_point = point as c_int;
        symbols::rl_end = line.len() as c_int;
        if !previous.is_null() {
            symbols::xfree(previous as *mut std::ffi::c_void);
        }
    }
}

/// The shell's `$COMP_WORDBREAKS`, or readline's default when it is unset.
pub fn word_breaks() -> String {
    unsafe { symbols::shell_variable("COMP_WORDBREAKS") }
        .unwrap_or_else(|| DEFAULT_WORD_BREAKS.to_string())
}

/// ```c
/// static char *bash_completer_word_break_characters = " \t\n\"'@><=;|&(:";
/// static char *bash_nohostname_word_break_characters = " \t\n\"'><=;|&(:";
/// ```
/// [`bashline.c:313`] — bash picks between them on `perform_hostname_completion`.
/// This is the second, which is what `$COMP_WORDBREAKS` normally holds; it is
/// only reached when that variable is unset.
///
/// [`bashline.c:313`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/bashline.c?h=bash-5.3#n313
const DEFAULT_WORD_BREAKS: &str = " \t\n\"'><=;|&(:";

/// Our copy of the break set.
static mut OUR_WORD_BREAKS: *mut c_char = std::ptr::null_mut();

/// Point bash's splitter at the break set.
///
/// # Safety
/// Calls bash's allocator. Must run on bash's thread.
unsafe fn publish_word_breaks(breaks: &str) {
    unsafe {
        let Ok(text) = CString::new(breaks) else {
            return;
        };
        let previous = OUR_WORD_BREAKS;
        OUR_WORD_BREAKS = symbols::bash_strdup(text.to_str().unwrap_or(DEFAULT_WORD_BREAKS));
        symbols::rl_completer_word_break_characters = OUR_WORD_BREAKS;
        if !previous.is_null() {
            symbols::xfree(previous as *mut std::ffi::c_void);
        }
    }
}

/// Strip the user-typed quoting, reporting the opening quote.
fn dequote(word: &str) -> (String, Option<u8>) {
    let quote = match find_quote_type(word) {
        Some(QuoteType::SingleQuote) => Some(b'\''),
        Some(QuoteType::DoubleQuote) => Some(b'"'),
        // Backslashes are stripped but readline reports no quote character.
        _ => None,
    };
    (dequoting_function_rust(word), quote)
}

/// How to read a match array's first element.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Convention {
    /// Every element is a candidate.
    Plain,
    /// What readline's generators produce. Element 0 is the longest common
    /// prefix, not a candidate. A single match comes back on its own instead.
    ///
    /// matching "ab", "ac"  ->  ["a", "ab", "ac"]
    /// matching "abc"            ->  ["abc"]
    CommonPrefixFirst,
}

/// Copy a `char **` out of bash and free it.
unsafe fn take_matches(matches: *mut *mut c_char, convention: Convention) -> Vec<String> {
    if matches.is_null() {
        return Vec::new();
    }
    unsafe {
        let mut all = Vec::new();
        let mut index = 0isize;
        loop {
            let entry = *matches.offset(index);
            if entry.is_null() {
                break;
            }
            all.push(CStr::from_ptr(entry).to_string_lossy().into_owned());
            index += 1;
        }
        symbols::strvec_dispose(matches);

        if convention == Convention::CommonPrefixFirst && all.len() > 1 {
            all.remove(0);
        }
        all
    }
}

/// Drop the trailing space bash-completion puts on a finished word.
fn strip_trailing_space(candidates: &mut [String]) {
    for candidate in candidates {
        if candidate.ends_with(' ') && !candidate.trim_end().is_empty() {
            candidate.pop();
        }
    }
}

/// Append `/` to candidates that name a directory.
///
/// Readline adds this while printing (bash never puts it in the string itself).
///
/// Reedline requires this to chain into the directory.
fn mark_directories(candidates: &mut [String]) {
    for candidate in candidates {
        if candidate.ends_with('/') {
            continue;
        }
        // `~/dev` is not a path until bash expands it.
        let path = match candidate.starts_with('~') {
            true => match unsafe { symbols::expand_tilde(candidate) } {
                Some(expanded) => expanded,
                None => continue,
            },
            false => candidate.clone(),
        };
        if std::path::Path::new(&path).is_dir() {
            candidate.push('/');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{assignment_prefix, dequote, in_command_position};

    #[test]
    fn a_command_follows_anything_that_ends_one() {
        for line in [
            "",
            "ls; ",
            "ls | ",
            "a && ",
            "( ",
            "{ ",
            "! ",
            "for i in 1; do\n",
        ] {
            assert!(in_command_position(line, line.len()), "{line:?}");
        }
        for line in ["ls ", "ls -", "cat file "] {
            assert!(!in_command_position(line, line.len()), "{line:?}");
        }
    }

    #[test]
    fn an_assignment_prefix_is_not_part_of_the_command() {
        for (line, command) in [
            ("FOO=bar widget a", "widget a"),
            ("A=1 B=2 widget a", "widget a"),
            ("arr[0]=x widget", "widget"),
            ("PATH+=:/opt widget", "widget"),
            ("FOO= widget", "widget"),
        ] {
            assert_eq!(&line[assignment_prefix(line)..], command, "{line:?}");
        }
    }

    #[test]
    fn a_word_that_only_looks_like_an_assignment_is_the_command() {
        // A name has to be a name, and `--opt=val` is an argument either way.
        for line in ["=nope x", "9lives=x cmd", "./a=b c", "widget --opt=val"] {
            assert_eq!(assignment_prefix(line), 0, "{line:?}");
        }
    }

    #[test]
    fn an_opening_quote_is_reported_and_removed() {
        assert_eq!(dequote("'my fi"), ("my fi".to_string(), Some(b'\'')));
        assert_eq!(dequote("\"my fi"), ("my fi".to_string(), Some(b'"')));
    }

    #[test]
    fn a_backslash_escape_is_the_character_it_escapes() {
        // `cat my\ fi` looks for a name already containing a space.
        assert_eq!(dequote(r"my\ fi"), ("my fi".to_string(), None));
        assert_eq!(dequote(r"a\$b"), ("a$b".to_string(), None));
    }

    #[test]
    fn a_candidate_is_deduped_after_it_is_rewritten() {
        // `echo` and `echo ` are two strings until the space is stripped.
        let mut candidates = vec!["echo ".to_string(), "echo".to_string()];
        super::strip_trailing_space(&mut candidates);
        super::dedupe(&mut candidates);
        assert_eq!(candidates, vec!["echo".to_string()]);
    }

    #[test]
    fn an_unquoted_word_is_unchanged() {
        assert_eq!(dequote("plain"), ("plain".to_string(), None));
        assert_eq!(dequote(""), (String::new(), None));
    }
}
