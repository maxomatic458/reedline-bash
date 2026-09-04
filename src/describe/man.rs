//! Extract command and option descriptions from a manpage.

use std::collections::HashMap;

/// What one page documents
pub type PageTable = HashMap<String, String>;

/// Descriptions are one line in a menu.
const MAX_DESCRIPTION_CHARS: usize = 1000;

/// This will probably never actually show because reedline is more likely to
/// truncate the line earlier.
const ELLIPSIS: char = '\u{2026}';

/// `name (section) - summary`, one per line.
pub fn parse_whatis(text: &str) -> Vec<(String, String)> {
    let mut best: HashMap<String, (bool, String)> = HashMap::new();
    for line in text.lines() {
        let Some((name, rest)) = line.split_once(" (") else {
            continue;
        };
        let Some((section, rest)) = rest.split_once(')') else {
            continue;
        };
        let Some((_, summary)) = rest.split_once("- ") else {
            continue;
        };
        // Commands live in sections 1 and 8.
        let is_command = matches!(section.chars().next(), Some('1' | '8'));
        let entry = best.entry(name.to_string());
        match entry {
            std::collections::hash_map::Entry::Occupied(mut seen)
                if is_command && !seen.get().0 =>
            {
                seen.insert((true, tidy(summary)));
            }
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert((is_command, tidy(summary)));
            }
            _ => {}
        }
    }
    best.into_iter()
        .map(|(name, (_, summary))| (name, summary))
        .collect()
}

/// `help -d '*'`: `name - summary`, after heading.
pub fn parse_help_d(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (name, summary) = line.split_once(" - ")?;
            (!name.is_empty() && !name.contains(char::is_whitespace))
                .then(|| (name.to_string(), tidy(summary)))
        })
        .collect()
}

/// One line, one sentence, no run of spaces.
fn tidy(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let sentence = match joined.find(". ") {
        Some(end) if end > 0 => &joined[..=end],
        _ => joined.as_str(),
    };
    let mut out: String = sentence.chars().take(MAX_DESCRIPTION_CHARS).collect();
    if out.len() < sentence.len() {
        out.push(ELLIPSIS);
    }
    out
}

/// Parses the options from a manpage.
/// ```text
///        -c count
///            Stop after sending count packets.
///        -a, --all                  do not ignore entries starting with .
///        --help Print help information.
/// ```
pub fn parse_options(text: &str) -> PageTable {
    let lines: Vec<(usize, String)> = text
        .lines()
        .map(strip_overstrike)
        .map(expand_tabs)
        .map(|line| (indent_of(&line), line))
        .collect();

    let mut table = PageTable::new();
    let mut i = 0;
    while i < lines.len() {
        let (indent, line) = &lines[i];
        i += 1;
        let Some((flags, mut description)) = parse_entry_head(line.trim()) else {
            continue;
        };

        // What follows at a deeper indent is the rest of the description.
        while let Some((deeper, more)) = lines.get(i) {
            if more.trim().is_empty() || deeper <= indent {
                break;
            }
            if !description.is_empty() {
                description.push(' ');
            }
            description.push_str(more.trim());
            i += 1;
        }
        if description.is_empty() {
            continue;
        }
        let description = tidy(&description);
        for flag in flags {
            table.entry(flag).or_insert_with(|| description.clone());
        }
    }
    table
}

/// The flags an entry line names, and whatever description it carries.
///
/// `-c count, --count=N   text`
fn parse_entry_head(line: &str) -> Option<(Vec<String>, String)> {
    let mut chars = line.chars();
    if chars.next() != Some('-')
        || chars
            .next()
            .is_none_or(|c| c.is_whitespace() || c == '-' && chars.next().is_none())
    {
        return None;
    }

    let (head, description) = match line.find("  ") {
        Some(at) => (&line[..at], line[at..].trim().to_string()),
        None => (line, String::new()),
    };

    let parts: Vec<&str> = head.split(',').map(str::trim).collect();
    let mut flags = Vec::new();
    let mut description = description;
    for (index, part) in parts.iter().enumerate() {
        let mut words = part.split_whitespace();
        let Some(flag) = words.next().and_then(flag_name) else {
            break;
        };
        flags.push(flag);

        let rest = without_placeholders(&words.collect::<Vec<_>>().join(" "));
        if index == parts.len() - 1
            && description.is_empty()
            && rest.split_whitespace().count() >= 2
        {
            description = rest;
        }
    }
    (!flags.is_empty()).then_some((flags, description))
}

/// `text` without its `<ARG>` and `[note]` groups.
fn without_placeholders(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '<' | '[' => depth += 1,
            '>' | ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Subcommands and what they do, from the `COMMANDS` or `SUBCOMMANDS` section.
///
/// ```text
/// COMMANDS
///        cargo-build(1)
///            Compile the current package.
///
///        attach
///            Attach to a running container
///
///        list-units [PATTERN...]
///            List units currently in memory.
/// ```
pub fn parse_subcommands(text: &str) -> PageTable {
    let lines: Vec<(usize, String)> = text
        .lines()
        .map(strip_overstrike)
        .map(expand_tabs)
        .map(|line| (indent_of(&line), line))
        .collect();

    let mut table = PageTable::new();
    let mut in_commands = false;
    let mut i = 0;
    while i < lines.len() {
        let (indent, line) = &lines[i];
        i += 1;
        if is_heading(line) {
            in_commands = line.contains("COMMANDS");
            continue;
        }
        if !in_commands {
            continue;
        }
        let Some((name, mut description)) = parse_subcommand_head(line.trim()) else {
            continue;
        };
        while let Some((deeper, more)) = lines.get(i) {
            if more.trim().is_empty() || deeper <= indent {
                break;
            }
            if !description.is_empty() {
                description.push(' ');
            }
            description.push_str(more.trim());
            i += 1;
        }
        if description.is_empty() {
            continue;
        }
        table.entry(name).or_insert_with(|| tidy(&description));
    }
    table
}

/// A section title: at the left margin, in capitals.
fn is_heading(line: &str) -> bool {
    !line.is_empty()
        && !line.starts_with(char::is_whitespace)
        && line.chars().all(|c| c.is_ascii_uppercase() || c == ' ')
}

/// The subcommand a line introduces, and any description it has.
///
/// The name is the first word: a plain one like `attach`, or a page reference
/// like `cargo-bench(1)`.
fn parse_subcommand_head(line: &str) -> Option<(String, String)> {
    let (head, description) = match line.find("  ") {
        Some(at) => (&line[..at], line[at..].trim().to_string()),
        None => (line, String::new()),
    };
    let first = head.split_whitespace().next()?;
    let name = match first.strip_suffix(')') {
        Some(reference) => {
            let (page, section) = reference.rsplit_once('(')?;
            if section.is_empty() || !section.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            page.split_once('-').map_or(page, |(_, name)| name)
        }
        None => first,
    };
    let is_name = name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.'));
    is_name.then(|| (name.to_string(), description))
}

/// `--count=N` and `-o[FILE]` are `--count` and `-o`; a bare dash is not a flag.
fn flag_name(token: &str) -> Option<String> {
    let name = token.split(['=', '[']).next().unwrap_or(token);
    let body = name.trim_start_matches('-');
    let dashes = name.len() - body.len();
    ((1..=2).contains(&dashes)
        && !body.is_empty()
        && body
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '+' | '.' | '#' | '?')))
    .then(|| name.to_string())
}

/// `x\x08x`: bold. `_\x08x`: underline.
fn strip_overstrike(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for c in line.chars() {
        if c == '\u{8}' {
            out.pop();
        } else {
            out.push(c);
        }
    }
    out
}

fn expand_tabs(line: String) -> String {
    if !line.contains('\t') {
        return line;
    }
    let mut out = String::with_capacity(line.len() + 8);
    let mut column = 0;
    for c in line.chars() {
        if c == '\t' {
            let width = 8 - column % 8;
            out.extend(std::iter::repeat_n(' ', width));
            column += width;
        } else {
            out.push(c);
            column += 1;
        }
    }
    out
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whatis_lines_become_summaries_and_commands_beat_library_functions() {
        let text = "\
printf (3)           - formatted output conversion
printf (1)           - Print output based off of the format string.
ping (8)             - send ICMP ECHO_REQUEST to network hosts
git-add (1)          - Add file contents to the index
junk line without the shape
";
        let map: HashMap<_, _> = parse_whatis(text).into_iter().collect();
        assert_eq!(map["ping"], "send ICMP ECHO_REQUEST to network hosts");
        assert_eq!(map["git-add"], "Add file contents to the index");
        assert_eq!(
            map["printf"], "Print output based off of the format string.",
            "section 1 wins over section 3 whichever comes first"
        );
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn help_d_lines_become_builtin_summaries() {
        let text = "\
Shell commands matching keyword `*'

! - Execute PIPELINE, which can be a simple command, and negate PIPELINE's
cd - Change the shell working directory.
echo - Write arguments to the standard output.
";
        let map: HashMap<_, _> = parse_help_d(text).into_iter().collect();
        assert_eq!(map["cd"], "Change the shell working directory.");
        assert_eq!(map["echo"], "Write arguments to the standard output.");
        assert!(map.contains_key("!"));
        assert!(!map.contains_key("Shell"));
    }

    #[test]
    fn a_page_with_the_description_below_the_option() {
        // ping(8) and git(1) are laid out like this.
        let text = "\
OPTIONS
       -3
           RTT precision (do not round up the result time).

       -c count
           Stop after sending count ECHO_REQUEST packets. With deadline
           option, ping waits for count ECHO_REPLY packets, until the timeout
           expires.

       -v, --version
           Prints the Git suite version that the git program came from.

           This paragraph belongs to the same option but is a new sentence.

       -C <path>
           Run as if git was started in <path>.
";
        let table = parse_options(text);
        assert_eq!(
            table["-3"],
            "RTT precision (do not round up the result time)."
        );
        assert_eq!(
            table["-c"],
            "Stop after sending count ECHO_REQUEST packets."
        );
        assert_eq!(table["-v"], table["--version"]);
        assert_eq!(table["-C"], "Run as if git was started in <path>.");
        assert!(!table.contains_key("<path>"));
        assert!(!table.contains_key("count"));
    }

    #[test]
    fn a_page_with_the_description_beside_the_option() {
        // GNU coreutils, and the uutils rewrite with its single space.
        let text = "\
DESCRIPTION
       List information about the FILEs (the current directory by default).

       -a, --all
              do not ignore entries starting with .

       -C     list entries by columns

       --color[=WHEN]
              color the output WHEN; more info below

       -w, --width=COLS
              set output width to COLS.  0 means no limit

       --help Print help information.
";
        let table = parse_options(text);
        assert_eq!(table["-a"], "do not ignore entries starting with .");
        assert_eq!(table["--all"], table["-a"]);
        assert_eq!(table["-C"], "list entries by columns");
        assert_eq!(table["--color"], "color the output WHEN; more info below");
        assert_eq!(table["--width"], "set output width to COLS.");
        assert_eq!(table["-w"], table["--width"]);
        assert_eq!(table["--help"], "Print help information.");
    }

    #[test]
    fn a_placeholder_and_a_default_beside_the_option_are_not_its_description() {
        // clap's generated pages, uv(1) here.
        let text = "\
OPTIONS
       -q, --quiet
              Use quiet output

       --color <COLOR_CHOICE> [default: auto]
              Control the use of color in output

       --cache-dir <CACHE_DIR> [env: UV_CACHE_DIR=]
              Path to the cache directory

       -h, --help
              Print help
";
        let table = parse_options(text);
        assert_eq!(table["--quiet"], "Use quiet output");
        assert_eq!(table["-q"], table["--quiet"]);
        assert_eq!(table["--color"], "Control the use of color in output");
        assert_eq!(table["--cache-dir"], "Path to the cache directory");
        assert_eq!(table["-h"], "Print help");
    }

    #[test]
    fn subcommands_listed_in_the_page_are_read_from_it() {
        let text = "\
SYNOPSIS
       uv [-q|--quiet] <subcommands>

SUBCOMMANDS
       uv-pip(1)
              Manage Python packages with a pip-compatible interface

       uv-venv(1)
              Create a virtual environment

COMMANDS
   Build Commands
       cargo-bench(1)
           Execute benchmarks of a package.

       attach
           Attach to a running container

           See docker-attach(1) for full documentation on the attach command.

       list-units [PATTERN...]
           List units currently in memory.

       run  Run a command in a new container

ENVIRONMENT
       not-a-command
           Text outside a commands section is not read.
";
        let table = parse_subcommands(text);
        assert_eq!(
            table["pip"],
            "Manage Python packages with a pip-compatible interface"
        );
        assert_eq!(table["venv"], "Create a virtual environment");
        assert_eq!(table["bench"], "Execute benchmarks of a package.");
        assert_eq!(table["attach"], "Attach to a running container");
        assert_eq!(table["list-units"], "List units currently in memory.");
        assert_eq!(table["run"], "Run a command in a new container");
        assert!(!table.contains_key("not-a-command"));
        assert!(!table.contains_key("uv"), "the synopsis is not a listing");
        assert_eq!(table.len(), 6, "{table:?}");
    }

    #[test]
    fn the_help_text_of_a_builtin_uses_tabs() {
        let text = "\
cd: cd [-L|[-P [-e]]] [-@] [dir]
    Change the shell working directory.

    Options:
      -L\tforce symbolic links to be followed: resolve symbolic
    \tlinks in DIR after processing instances of `..'
      -P\tuse the physical directory structure without following
    \tsymbolic links

    Exit Status:
    Returns 0 if the directory is changed.
";
        let table = parse_options(text);
        assert_eq!(
            table["-L"],
            "force symbolic links to be followed: resolve symbolic links in DIR after processing instances of `..'"
        );
        assert_eq!(
            table["-P"],
            "use the physical directory structure without following symbolic links"
        );
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn prose_and_dashes_that_are_not_options_are_left_alone() {
        let text = "\
       - a bullet point, not an option
       --
           Ends the options.
       -
           Standard input.
       --verbose
           Say more.
";
        let table = parse_options(text);
        assert_eq!(table.keys().collect::<Vec<_>>(), vec!["--verbose"]);
    }

    #[test]
    fn overstruck_bold_and_underline_are_stripped() {
        let text =
            "       -\u{8}--\u{8}-a\u{8}al\u{8}ll\u{8}l\n           _\u{8}A_\u{8}l_\u{8}l of it.\n";
        let table = parse_options(text);
        assert_eq!(table["--all"], "All of it.");
    }

    #[test]
    fn a_description_is_one_sentence_and_not_too_long() {
        assert_eq!(tidy("First   sentence.  Second one."), "First sentence.");
        assert_eq!(tidy("No period here"), "No period here");
        let long = "x".repeat(MAX_DESCRIPTION_CHARS + 200);
        let cut = tidy(&long);
        assert_eq!(cut.chars().count(), MAX_DESCRIPTION_CHARS + 1);
        assert!(cut.ends_with(ELLIPSIS));
    }
}
