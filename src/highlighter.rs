//! minimal bash-shaped syntax highlighter.

use nu_ansi_term::Style;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};

use flash::lexer::TokenKind;
use reedline::{Highlighter, StyledText};

use crate::grammar::dparser::{AnnotatedToken, DParser};

use crate::config::Palette;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    Command,
    Builtin,
    String,
    Comment,
    Variable,
    Operator,
}

impl Kind {
    fn style(self, palette: &Palette) -> Style {
        match self {
            Kind::Plain => Style::new(),
            Kind::Command => palette.command,
            Kind::Builtin => palette.builtin,
            Kind::String => palette.string,
            Kind::Comment => palette.comment,
            Kind::Variable => palette.variable,
            Kind::Operator => palette.operator,
        }
    }
}

const BUILTINS: &[&str] = &[
    "cd",
    "echo",
    "printf",
    "read",
    "export",
    "unset",
    "alias",
    "unalias",
    "source",
    ".",
    "eval",
    "exec",
    "exit",
    "return",
    "local",
    "declare",
    "typeset",
    "let",
    "shift",
    "test",
    "set",
    "shopt",
    "trap",
    "wait",
    "jobs",
    "fg",
    "bg",
    "kill",
    "history",
    "bind",
    "complete",
    "compgen",
    "pushd",
    "popd",
    "dirs",
    "type",
    "command",
    "builtin",
    "true",
    "false",
    "mapfile",
    "readarray",
];

#[derive(Default)]
pub struct BashHighlighter {
    palette: Palette,
    /// Set if scanning ever panicked.
    failed: AtomicBool,
}

impl BashHighlighter {
    pub fn new(palette: Palette) -> Self {
        BashHighlighter {
            palette,
            failed: AtomicBool::new(false),
        }
    }
}

/// Hands the line back unstyled, for `highlight = false`.
#[derive(Default)]
pub struct PlainHighlighter;

impl Highlighter for PlainHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        unstyled(line)
    }
}

/// The highlighter the configuration asks for.
pub fn for_config(enabled: bool, palette: &Palette) -> Box<dyn Highlighter> {
    if enabled {
        Box::new(BashHighlighter::new(palette.clone()))
    } else {
        Box::new(PlainHighlighter)
    }
}

impl Highlighter for BashHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        if self.failed.load(Ordering::Relaxed) {
            return unstyled(line);
        }
        match catch_unwind(AssertUnwindSafe(|| self.scan(line))) {
            Ok(styled) => styled,
            Err(_) => {
                self.failed.store(true, Ordering::Relaxed);
                unstyled(line)
            }
        }
    }
}

/// The line in one run, with no styling at all.
fn unstyled(line: &str) -> StyledText {
    let mut out = StyledText::new();
    if !line.is_empty() {
        out.push((Style::new(), line.to_string()));
    }
    out
}

impl BashHighlighter {
    fn scan(&self, line: &str) -> StyledText {
        let mut out = StyledText::new();
        let mut pending: Option<(Kind, String)> = None;

        let tokens = DParser::parse_and_annotate(line);
        let expansions = expansion_mask(&tokens);

        for (token, &in_expansion) in tokens.iter().zip(&expansions) {
            let kind = classify(token, in_expansion);
            match &mut pending {
                Some((open, text)) if *open == kind => text.push_str(&token.token.value),
                Some((open, text)) => {
                    out.push((open.style(&self.palette), std::mem::take(text)));
                    pending = Some((kind, token.token.value.clone()));
                }
                None => pending = Some((kind, token.token.value.clone())),
            }
        }
        if let Some((kind, text)) = pending {
            out.push((kind.style(&self.palette), text));
        }
        out
    }
}

/// Tokens belonging to an expansion are styled as one.
///
/// `${...}` everything between the braces is part of the expansion -> same style.
/// `$(...)` inner command gets its own style.
fn expansion_mask(tokens: &[AnnotatedToken]) -> Vec<bool> {
    use TokenKind as T;
    let mut mask = vec![false; tokens.len()];
    // One entry per open expansion; `true` for the opaque `${`.
    let mut open: Vec<bool> = Vec::new();

    for (i, token) in tokens.iter().enumerate() {
        match token.token.kind {
            T::ParamExpansion => {
                mask[i] = true;
                open.push(true);
            }
            T::CmdSubst | T::ArithSubst => {
                mask[i] = true;
                open.push(false);
            }
            // A `}` or `)` without opening is ignored,
            T::RBrace | T::RParen | T::DoubleRParen if !open.is_empty() => {
                open.pop();
                mask[i] = true;
            }
            _ => mask[i] = open.last() == Some(&true),
        }
    }
    mask
}

/// Which colour a token gets.
fn classify(token: &AnnotatedToken, in_expansion: bool) -> Kind {
    use TokenKind as T;
    let notes = &token.annotations;

    if notes.is_comment || matches!(token.token.kind, T::Comment) {
        return Kind::Comment;
    }
    if in_expansion {
        return Kind::Variable;
    }
    if notes.is_env_var {
        return Kind::Variable;
    }
    if notes.is_inside_single_quotes || notes.is_inside_double_quotes {
        return Kind::String;
    }

    match token.token.kind {
        T::SingleQuote | T::Quote | T::Backtick => Kind::String,
        T::Dollar | T::CmdSubst | T::ArithSubst | T::ParamExpansion | T::ParamExpansionOp(_) => {
            Kind::Variable
        }
        T::If
        | T::Then
        | T::Elif
        | T::Else
        | T::Fi
        | T::Case
        | T::Esac
        | T::For
        | T::While
        | T::Until
        | T::Do
        | T::Done
        | T::In
        | T::Select
        | T::Function => Kind::Builtin,
        T::Pipe
        | T::And
        | T::Or
        | T::Semicolon
        | T::DoubleSemicolon
        | T::Background
        | T::Assignment
        | T::Less
        | T::Great
        | T::DGreat
        | T::InputDup
        | T::OutputDup
        | T::ReadWrite
        | T::Clobber
        | T::HereString
        | T::LParen
        | T::RParen
        | T::DoubleRParen
        | T::LBrace
        | T::RBrace => Kind::Operator,
        T::Word(_) if notes.command_word.is_some() => {
            if BUILTINS.contains(&token.token.value.as_str()) {
                Kind::Builtin
            } else {
                Kind::Command
            }
        }
        _ => Kind::Plain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(line: &str) -> String {
        BashHighlighter::default()
            .highlight(line, line.len())
            .buffer
            .iter()
            .map(|(_, s)| s.as_str())
            .collect()
    }

    fn kinds(line: &str) -> Vec<(String, Style)> {
        BashHighlighter::default()
            .highlight(line, line.len())
            .buffer
            .into_iter()
            .map(|(style, s)| (s, style))
            .collect()
    }

    /// Adjacent tokens of one kind are emitted as a single run, so a needle can
    /// sit inside a run rather than being one: `git commit -m` yields `"git"`
    /// and `" commit -m "`.
    fn style_of<'a>(pieces: &'a [(String, Style)], needle: &str) -> Option<&'a Style> {
        pieces
            .iter()
            .find(|(s, _)| s == needle || s.trim() == needle)
            .or_else(|| {
                pieces
                    .iter()
                    .find(|(s, _)| s.split_whitespace().any(|word| word == needle))
            })
            .map(|(_, st)| st)
    }

    #[test]
    fn highlighting_is_lossless() {
        for line in [
            "",
            " ",
            "echo hello",
            "git commit -m 'a message'",
            r#"echo "double $VAR quoted""#,
            "for i in 1 2 3; do echo $i; done",
            "ls -la | grep foo > out.txt # trailing comment",
            "echo $(date) ${HOME} $((1+2))",
            "echo 'unterminated",
            r"echo a\ b",
            "echo äöü 🚀",
            r#"echo "outer 'inner' done""#,
            r#"echo 'outer "inner" done'"#,
            "echo $(echo $(echo deep))",
            "echo ${VAR:-$(hostname)}",
            "echo $(( (1 + 2) * 3 ))",
            "cmd 2>&1 >>log <in 2>/dev/null",
            "cat <<< 'here string'",
            "a |& b",
            "( cd /tmp && ls )",
            "{ echo a; echo b; }",
            "if ! grep -q pattern file; then echo missing; fi",
            r"echo \$notvar \#nothash \'noquote \\",
            r"echo a\ b\ c",
            "for f in *.txt; do\n  echo \"$f\"\ndone",
            "case $x in\n  a) echo one;;\n  *) echo other;;\nesac",
            "find . -name '*.rs' -print0 | xargs -0 grep -l TODO | sort -u > /tmp/out.txt # notes",
            "echo \"$(echo \"$(echo nested)\")\"",
            "((i++))",
            "echo äöü$VAR'ü'\"ö\"",
        ] {
            assert_eq!(text_of(line), line, "round trip failed for {line:?}");
        }
    }

    #[test]
    fn no_input_can_make_the_scanner_run_off_the_end() {
        for line in [
            "$",
            "${",
            "$(",
            "$((",
            "${VAR",
            "$(cmd",
            "\"",
            "'",
            r"\",
            "#",
            "echo $",
            "echo ${VAR:-",
            "echo \"$(",
            "|",
            ">",
            "2>&",
            "ä",
            "$ä",
            "\u{1b}",
        ] {
            assert_eq!(text_of(line), line, "round trip failed for {line:?}");
        }
    }

    #[test]
    fn the_command_word_is_coloured_and_arguments_are_not() {
        let pieces = kinds("git status");
        assert_eq!(
            style_of(&pieces, "git"),
            Some(&Kind::Command.style(&Palette::default()))
        );
        assert_eq!(
            style_of(&pieces, "status"),
            Some(&Kind::Plain.style(&Palette::default()))
        );
    }

    #[test]
    fn a_command_after_a_pipe_is_a_command_again() {
        let pieces = kinds("ls | grep foo");
        assert_eq!(
            style_of(&pieces, "ls"),
            Some(&Kind::Command.style(&Palette::default()))
        );
        assert_eq!(
            style_of(&pieces, "grep"),
            Some(&Kind::Command.style(&Palette::default()))
        );
        assert_eq!(
            style_of(&pieces, "foo"),
            Some(&Kind::Plain.style(&Palette::default()))
        );
    }

    #[test]
    fn a_redirection_target_is_not_a_command() {
        let pieces = kinds("ls > out.txt");
        assert_eq!(
            style_of(&pieces, "out.txt"),
            Some(&Kind::Plain.style(&Palette::default()))
        );
    }

    #[test]
    fn quotes_variables_and_comments_get_their_own_styles() {
        let pieces = kinds("echo 'lit' $HOME # note");
        assert_eq!(
            style_of(&pieces, "'lit'"),
            Some(&Kind::String.style(&Palette::default()))
        );
        assert_eq!(
            style_of(&pieces, "$HOME"),
            Some(&Kind::Variable.style(&Palette::default()))
        );
        assert_eq!(
            style_of(&pieces, "# note"),
            Some(&Kind::Comment.style(&Palette::default()))
        );
    }

    #[test]
    fn builtins_and_keywords_are_distinguishable_from_commands() {
        let pieces = kinds("cd /tmp");
        assert_eq!(
            style_of(&pieces, "cd"),
            Some(&Kind::Builtin.style(&Palette::default()))
        );
        let pieces = kinds("for i in 1; do echo hi; done");
        assert_eq!(
            style_of(&pieces, "for"),
            Some(&Kind::Builtin.style(&Palette::default()))
        );

        let pieces = kinds("if grep -q x f; then echo y; fi");
        assert_eq!(
            style_of(&pieces, "grep"),
            Some(&Kind::Command.style(&Palette::default()))
        );
    }

    #[test]
    fn a_panic_while_scanning_falls_back_to_an_unstyled_line() {
        struct Exploding(BashHighlighter);
        impl Highlighter for Exploding {
            fn highlight(&self, line: &str, cursor: usize) -> StyledText {
                if self.0.failed.load(Ordering::Relaxed) {
                    return unstyled(line);
                }
                match catch_unwind(AssertUnwindSafe(|| panic!("scanner bug"))) {
                    Ok(()) => self.0.highlight(line, cursor),
                    Err(_) => {
                        self.0.failed.store(true, Ordering::Relaxed);
                        unstyled(line)
                    }
                }
            }
        }

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let h = Exploding(BashHighlighter::default());
        let styled = h.highlight("echo hello", 10);
        std::panic::set_hook(previous);

        // The text survives, unstyled
        let text: String = styled.buffer.iter().map(|(_, s)| s.as_str()).collect();
        assert_eq!(text, "echo hello");
        assert_eq!(styled.buffer.len(), 1);
        assert_eq!(styled.buffer[0].0, Style::new());

        assert!(h.0.failed.load(Ordering::Relaxed));
    }
    #[test]
    fn every_prefix_of_a_script_is_survivable() {
        let scripts = [
            "deploy() {\n  local t=$1\n  if [ -z \"$t\" ]; then\n    echo 'usage' >&2\n  fi\n}",
            "for f in *.log; do\n  case \"$f\" in\n    *.gz) gunzip -c \"$f\" ;;\n  esac\ndone",
            "A=1 B=$(date) cmd --opt=v 'str' \"d $x\" # note",
            "echo $((1+2)) ${a[@]} $'raw' <<<'here' 2>&1 |& tee ä🚀",
        ];
        for script in scripts {
            for end in 0..=script.len() {
                if !script.is_char_boundary(end) {
                    continue;
                }
                let prefix = &script[..end];
                assert_eq!(text_of(prefix), prefix, "prefix {end} of {script:?}");
            }
        }
    }

    #[test]
    fn a_brace_group_keeps_the_command_inside_it() {
        let pieces = kinds("{ echo a; ls; }");
        let p = Palette::default();
        assert_eq!(style_of(&pieces, "{"), Some(&p.operator));
        assert_eq!(style_of(&pieces, "}"), Some(&p.operator));
        assert_eq!(style_of(&pieces, "echo"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "ls"), Some(&p.command));
    }

    #[test]
    #[ignore = "dparser: `!` is a plain word and takes the command slot from the command after it"]
    fn a_negation_keeps_the_command_after_it() {
        let pieces = kinds("if ! grep -q x f; then :; fi");
        let p = Palette::default();
        assert_eq!(style_of(&pieces, "!"), Some(&p.operator));
        assert_eq!(style_of(&pieces, "grep"), Some(&p.command));
    }

    #[test]
    #[ignore = "dparser: `(` and `{` take the command slot from the command inside them"]
    fn command_position_follows_the_separators() {
        let p = Palette::default();
        let plain = Style::new();
        for (line, word, expected, note) in [
            ("( ls )", "ls", &p.command, "after ("),
            ("a && ls", "ls", &p.command, "after &&"),
            ("a || ls", "ls", &p.command, "after ||"),
            ("a; ls", "ls", &p.command, "after ;"),
            ("a & ls", "ls", &p.command, "after &"),
            ("a |& ls", "ls", &p.command, "after |&"),
            (
                "echo > ls",
                "ls",
                &plain,
                "a redirection target is not a command",
            ),
            ("echo >> ls", "ls", &plain, "nor after >>"),
            ("echo < ls", "ls", &plain, "nor after <"),
        ] {
            let pieces = kinds(line);
            assert_eq!(style_of(&pieces, word), Some(expected), "{note}: {line:?}");
        }
    }

    #[test]
    fn a_quote_of_the_other_kind_stays_inside_the_string() {
        let p = Palette::default();
        for line in [
            r#"echo "outer 'inner' done""#,
            r#"echo 'outer "inner" done'"#,
        ] {
            let pieces = kinds(line);
            let quoted: Vec<&String> = pieces
                .iter()
                .filter(|(_, st)| *st == p.string)
                .map(|(s, _)| s)
                .collect();
            assert_eq!(quoted.len(), 1, "{line:?} should be one string run");
        }
    }

    #[test]
    fn a_parameter_expansion_is_opaque_but_a_substitution_is_not() {
        let p = Palette::default();

        // `${...}` is one run however deeply it nests.
        for (line, expected) in [
            ("echo ${#ARR[@]}", "${#ARR[@]}"),
            ("echo ${VAR:-fallback}", "${VAR:-fallback}"),
        ] {
            let pieces = kinds(line);
            assert_eq!(style_of(&pieces, expected), Some(&p.variable), "{line:?}");
        }

        // `$(...)` holds a real command, which keeps its own colours.
        let pieces = kinds("echo $(echo $(date))");
        assert_eq!(style_of(&pieces, "$("), Some(&p.variable));
        assert_eq!(style_of(&pieces, "date"), Some(&p.command));
        assert_eq!(style_of(&pieces, "))"), Some(&p.variable));

        let pieces = kinds("echo ${VAR:-$(hostname)}");
        assert_eq!(style_of(&pieces, "${VAR:-$("), Some(&p.variable));
        assert_eq!(style_of(&pieces, "hostname"), Some(&p.command));
        assert_eq!(style_of(&pieces, ")}"), Some(&p.variable));
    }

    #[test]
    fn a_hash_inside_a_word_is_not_a_comment() {
        let p = Palette::default();
        for (line, not_comment) in [
            ("echo a#b", "a#b"),
            ("echo 'a # b'", "'a # b'"),
            (r"echo \#nothash", r"\#nothash"),
        ] {
            let pieces = kinds(line);
            let comments: Vec<&String> = pieces
                .iter()
                .filter(|(_, st)| *st == p.comment)
                .map(|(s, _)| s)
                .collect();
            assert!(
                comments.is_empty(),
                "{line:?} found comment in {not_comment}"
            );
        }
        // But it does after whitespace.
        let pieces = kinds("ls # a real comment");
        assert_eq!(style_of(&pieces, "# a real comment"), Some(&p.comment));
    }

    #[test]
    fn an_escape_defuses_the_character_after_it() {
        let p = Palette::default();
        let pieces = kinds(r"echo \$notvar");
        assert!(
            pieces.iter().all(|(_, st)| *st != p.variable),
            "escaped $ became an expansion: {pieces:?}"
        );
        let pieces = kinds(r"echo \'notquote");
        assert!(
            pieces.iter().all(|(_, st)| *st != p.string),
            "escaped quote opened a string: {pieces:?}"
        );
    }
    #[test]
    fn each_line_of_a_multiline_command_is_scanned_on_its_own() {
        let p = Palette::default();
        let pieces = kinds("for f in a b; do\n  echo $f\ndone");
        assert_eq!(style_of(&pieces, "for"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "do"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "echo"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "$f"), Some(&p.variable));
        assert_eq!(style_of(&pieces, "done"), Some(&p.builtin));
    }

    #[test]
    #[ignore = "dparser: an indexed target is not an env var, and `FOO=$BAR` swallows the command after it"]
    fn an_assignment_prefix_is_not_the_command() {
        let p = Palette::default();
        // The name being assigned to is the variable; the value is a plain word.
        for (line, assignment, command) in [
            ("FOO=bar echo hi", "FOO=", "echo"),
            ("A=1 B=2 cmd x", "B=", "cmd"),
            ("arr[0]=x cmd", "arr[0]=", "cmd"),
            ("PATH+=:/opt cmd", "PATH+=", "cmd"),
        ] {
            let pieces = kinds(line);
            assert_eq!(style_of(&pieces, assignment), Some(&p.variable), "{line:?}");
            let style = style_of(&pieces, command);
            assert!(
                style == Some(&p.command) || style == Some(&p.builtin),
                "{line:?}: {command} should be the command"
            );
        }

        // An expansion in the value does not end the prefix.
        let pieces = kinds("FOO=$BAR ls");
        assert_eq!(style_of(&pieces, "ls"), Some(&p.command));

        // A leading `=` has no name in front of it, so nothing is assigned.
        let pieces = kinds("=nope");
        assert!(
            !pieces.iter().any(|(_, style)| *style == p.variable),
            "{pieces:?}"
        );
    }

    #[test]
    #[ignore = "dparser: `for`/`select`/`case` give the command slot to their variable, and `time` is not a keyword"]
    fn a_keyword_that_takes_a_word_does_not_start_a_command() {
        let p = Palette::default();
        let plain = Style::new();
        for (line, word) in [
            ("for i in 1 2 3; do :; done", "i"),
            ("for i in 1 2 3; do :; done", "1"),
            ("case $x in a) :;; esac", "a"),
            ("select o in a b; do :; done", "o"),
        ] {
            let pieces = kinds(line);
            assert_eq!(style_of(&pieces, word), Some(&plain), "{line:?}: {word}");
        }

        // While the ones that do take a command still work.
        for (line, word) in [
            ("while read -r l; do :; done", "read"),
            ("if grep -q x f; then :; fi", "grep"),
            ("time ls", "ls"),
        ] {
            let pieces = kinds(line);
            let style = style_of(&pieces, word);
            assert!(
                style == Some(&p.command) || style == Some(&p.builtin),
                "{line:?}: {word} should be the command, got {style:?}"
            );
        }
    }

    #[test]
    fn a_newline_ends_a_command_like_a_semicolon_does() {
        let p = Palette::default();
        // The same thing on one line already works; the newline is the difference.
        for line in [
            "echo hi; if true; then :; fi",
            "echo hi\nif true; then :; fi",
        ] {
            let pieces = kinds(line);
            assert_eq!(style_of(&pieces, "if"), Some(&p.builtin), "{line:?}");
            assert_eq!(style_of(&pieces, "fi"), Some(&p.builtin), "{line:?}");
        }
    }

    #[test]
    fn a_function_body_written_over_several_lines_is_highlighted() {
        let script = "\
hello() {
    if [ -z \"$1\" ]; then
        echo \"Hello, stranger!\"
    else
        echo \"Hello, $1!\"
    fi
}";
        let p = Palette::default();
        let pieces = kinds(script);

        assert_eq!(text_of(script), script, "round trip");
        assert_eq!(style_of(&pieces, "hello"), Some(&p.command));
        assert_eq!(style_of(&pieces, "if"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "then"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "echo"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "else"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "fi"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "\"Hello, stranger!\""), Some(&p.string));
    }

    #[test]
    #[ignore = "dparser: `[` is not a command, and `local x=1` is one plain word"]
    fn a_multiline_function_definition_is_coloured_throughout() {
        let script = "\
deploy() {
  local target=$1
  if [ -z \"$target\" ]; then
    echo 'usage: deploy TARGET' >&2
    return 1
  fi
  rsync -az --delete ./build/ \"$target:/srv/app\" # push it
}";
        let p = Palette::default();
        let plain = Style::new();
        let pieces = kinds(script);

        assert_eq!(text_of(script), script, "round trip");
        assert_eq!(style_of(&pieces, "deploy"), Some(&p.command));
        assert_eq!(style_of(&pieces, "{"), Some(&p.operator));
        assert_eq!(style_of(&pieces, "local"), Some(&p.builtin));
        // `local` declares, so its whole argument is the assignment.
        assert_eq!(style_of(&pieces, "target=$1"), Some(&p.variable));
        assert_eq!(style_of(&pieces, "if"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "["), Some(&p.command));
        assert_eq!(style_of(&pieces, "-z"), Some(&plain));
        // An expansion inside a string keeps its own colour.
        assert_eq!(style_of(&pieces, "\""), Some(&p.string));
        assert_eq!(style_of(&pieces, "$target"), Some(&p.variable));
        assert_eq!(style_of(&pieces, "then"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "echo"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "'usage: deploy TARGET'"), Some(&p.string));
        assert_eq!(style_of(&pieces, ">&"), Some(&p.operator));
        assert_eq!(style_of(&pieces, "return"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "fi"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "rsync"), Some(&p.command));
        assert_eq!(style_of(&pieces, "--delete"), Some(&plain));
        assert_eq!(style_of(&pieces, "# push it"), Some(&p.comment));
        assert_eq!(style_of(&pieces, "}"), Some(&p.operator));
    }

    #[test]
    #[ignore = "dparser: a nested `case` is not a keyword, and `COUNT=$(...)` is one plain word"]
    fn a_multiline_loop_and_case_script_is_coloured_throughout() {
        let script = "\
for f in *.log; do
  case \"$f\" in
    *.gz) gunzip -c \"$f\" | grep -c ERROR ;;
    *)    COUNT=$(grep -c ERROR \"$f\") && echo \"$f: $COUNT\" ;;
  esac
done | sort -rn | head -20";
        let p = Palette::default();
        let plain = Style::new();
        let pieces = kinds(script);

        assert_eq!(text_of(script), script, "round trip");
        assert_eq!(style_of(&pieces, "for"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "f"), Some(&plain), "a loop variable name");
        assert_eq!(style_of(&pieces, "in"), Some(&p.builtin));
        assert_eq!(
            style_of(&pieces, "*.log"),
            Some(&plain),
            "a word list entry"
        );
        assert_eq!(style_of(&pieces, "do"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "case"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "*.gz"), Some(&plain), "a case pattern");
        assert_eq!(style_of(&pieces, "gunzip"), Some(&p.command));
        assert_eq!(style_of(&pieces, "|"), Some(&p.operator));
        assert_eq!(style_of(&pieces, "grep"), Some(&p.command));
        assert_eq!(style_of(&pieces, ";;"), Some(&p.operator));
        // The substitution's delimiters are the expansion; what is inside is a
        // command like any other.
        assert_eq!(style_of(&pieces, "$("), Some(&p.variable));
        assert_eq!(style_of(&pieces, "COUNT=$("), Some(&p.variable));
        assert_eq!(
            style_of(&pieces, "*"),
            Some(&plain),
            "the catch-all pattern"
        );
        assert_eq!(style_of(&pieces, "&&"), Some(&p.operator));
        assert_eq!(style_of(&pieces, "esac"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "done"), Some(&p.builtin));
        assert_eq!(style_of(&pieces, "sort"), Some(&p.command));
        assert_eq!(style_of(&pieces, "head"), Some(&p.command));
    }

    #[test]
    fn a_realistic_pipeline_colours_every_part() {
        let line = "find . -name '*.rs' | xargs grep -l TODO > $HOME/out.txt # notes";
        let p = Palette::default();
        let pieces = kinds(line);
        assert_eq!(text_of(line), line);
        assert_eq!(style_of(&pieces, "find"), Some(&p.command));
        assert_eq!(style_of(&pieces, "'*.rs'"), Some(&p.string));
        assert_eq!(style_of(&pieces, "|"), Some(&p.operator));
        assert_eq!(style_of(&pieces, "xargs"), Some(&p.command));
        assert_eq!(style_of(&pieces, ">"), Some(&p.operator));
        assert_eq!(style_of(&pieces, "$HOME"), Some(&p.variable));
        assert_eq!(style_of(&pieces, "# notes"), Some(&p.comment));
    }

    #[test]
    fn the_plain_highlighter_returns_the_line_untouched_and_unstyled() {
        for line in ["", "echo hi", "for i in 1; do echo $i; done"] {
            let styled = PlainHighlighter.highlight(line, line.len());
            let text: String = styled.buffer.iter().map(|(_, s)| s.as_str()).collect();
            assert_eq!(text, line);
            assert!(
                styled
                    .buffer
                    .iter()
                    .all(|(style, _)| *style == Style::new())
            );
        }
    }

    #[test]
    fn unterminated_constructs_do_not_panic_or_truncate() {
        for line in ["echo \"open", "echo ${unclosed", "echo $(", "echo 'x", "$"] {
            assert_eq!(text_of(line), line, "{line:?}");
        }
    }
}
