//! [Reedline](https://crates.io/crates/reedline) as bash's line editor.
//!
//! ```console
//! $ enable -f /path/to/libreedline_bash.so reedline
//! ```
//!
//! # How it works
//!
//! ```text
//!   bash parser  ->  get_char (inserted by us)  ->  reedline  ->  the line  ->  bash parser
//! ```
//!
//! 1. `enable -f` loads this object into the bash process and looks up
//!    [`reedline_struct`], [`reedline_builtin_load`] and
//!    [`reedline_builtin_unload`] by name.
//! 2. When loaded we point `bash_input.getter` at `get_char`. bash will then
//!    read its input through that pointer (bypassing readline). See [`bash::input`].
//! 3. Bash asks for one character at a time. When the buffer is empty,
//!    `LineFeeder` calls `Reedline::read_line()`.
//! 4. Reedline draws the prompt, handles the editing and returns the line.
//! 5. We feed it into the bash parser byte by byte, with a `\n` on the end.
//!
//! Reedline uses bash's native history ([`history::BashHistory`]) and completions
//! ([`bash::complete::candidates`]).

pub mod bash;
mod cli;
mod completer;
mod config;
mod describe;
mod editor;
mod grammar;
mod highlighter;
pub mod history;
mod keys;
mod process;
mod prompt;
mod style;
mod validator;
mod words;

use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

use crossterm::event::DisableBracketedPaste;

use bash::builtin::{DocLines, guard};
use bash::input::LineFeeder;
use bash::symbols;
use editor::Editor as LineEditor;

/// The editor, once bash loaded `libreedline_bash.so`.
static EDITOR: Mutex<Option<Editor>> = Mutex::new(None);

type Feeder = LineFeeder<Box<dyn FnMut() -> Option<String> + Send>>;

struct Editor {
    feeder: Feeder,
}

impl Editor {
    fn new() -> Self {
        // The editor is built at the first prompt, because bash may still load
        // its config & history.
        let mut line_editor: Option<LineEditor> = None;
        let fetch: Box<dyn FnMut() -> Option<String> + Send> = Box::new(move || {
            let editor = line_editor.get_or_insert_with(|| {
                let (editor, warnings) = LineEditor::new();
                for warning in &warnings {
                    eprintln!("reedline-bash: {warning}");
                }
                editor
            });
            editor.read_line()
        });
        Editor {
            feeder: LineFeeder::new(fetch),
        }
    }
}

extern "C" fn get_char() -> c_int {
    let read = std::panic::catch_unwind(|| {
        // A poisoned lock means an earlier call panicked.
        let mut slot = EDITOR.lock().ok()?;
        Some(
            slot.as_mut()
                .map_or(symbols::EOF, |editor| editor.feeder.next_char()),
        )
    });
    match read {
        Ok(Some(c)) => c,
        _ => hand_back_to_readline(),
    }
}

/// The editor failed. Return control to readline
fn hand_back_to_readline() -> c_int {
    eprintln!("reedline-bash: the editor failed, falling back to readline");
    *EDITOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    let _ = crossterm::execute!(std::io::stdout(), DisableBracketedPaste);
    unsafe { bash::input::restore() };
    c_int::from(b'\n')
}

extern "C" fn unget_char(c: c_int) -> c_int {
    guard(c, || match EDITOR.lock() {
        Ok(mut editor) => match editor.as_mut() {
            Some(editor) => editor.feeder.unget(c),
            None => c,
        },
        Err(_) => c,
    })
}

/// The `reedline` command itself. See `cli`.
extern "C" fn call(words: *const symbols::WordList) -> c_int {
    guard(1, || {
        let mut argv = vec!["reedline".to_string()];
        argv.extend(unsafe { symbols::word_list(words) });
        let matches = match cli::command().try_get_matches_from(argv) {
            Ok(matches) => matches,
            // `--help` and `--version` arrive here too, and are not failures.
            Err(err) => {
                let _ = err.print();
                return if err.use_stderr() { 2 } else { 0 };
            }
        };
        match matches.subcommand() {
            Some(("complete", sub)) => {
                let line = sub.get_one::<String>("line").cloned().unwrap_or_default();
                let point = line.len();
                let start =
                    words::word_start_with_breaks(&line, point, &bash::complete::word_breaks());
                for candidate in unsafe { bash::complete::candidates(&line, start, point) }.matches
                {
                    println!("{candidate}");
                }
                0
            }
            Some(("clear-cache", _)) => clear_cache(),
            Some(("install-man", sub)) => install_man(sub.get_one::<std::path::PathBuf>("dir")),
            _ => {
                println!("reedline-bash {} (loaded)", env!("CARGO_PKG_VERSION"));
                0
            }
        }
    })
}

/// `reedline clear-cache`.
fn clear_cache() -> c_int {
    let Some(dir) = describe::cache_dir() else {
        eprintln!("reedline-bash: no cache directory: neither XDG_CACHE_HOME nor HOME is set");
        return 1;
    };
    let (files, bytes) = match describe::clear_cache_dir(&dir) {
        Ok(cleared) => cleared,
        Err(err) => {
            eprintln!("reedline-bash: cannot clear {}: {err}", dir.display());
            return 1;
        }
    };

    println!(
        "reedline-bash: cleared {files} cached pages ({bytes} bytes) from {}",
        dir.display()
    );
    0
}

/// The manpages, generated by `build.rs` from `cli::command()`.
mod manpages {
    include!(concat!(env!("OUT_DIR"), "/manpages.rs"));
}

/// `reedline install-man [--dir DIR]`.
fn install_man(dir: Option<&std::path::PathBuf>) -> c_int {
    let dir = match dir {
        Some(dir) => dir.clone(),
        None => match user_man1_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("reedline-bash: neither XDG_DATA_HOME nor HOME is set: Use --dir");
                return 1;
            }
        },
    };
    if let Err(err) = std::fs::create_dir_all(&dir) {
        eprintln!("reedline-bash: cannot create {}: {err}", dir.display());
        return 1;
    }
    for (name, page) in manpages::PAGES {
        let path = dir.join(name);
        if let Err(err) = std::fs::write(&path, page) {
            eprintln!("reedline-bash: cannot write {}: {err}", path.display());
            return 1;
        }
        println!("{}", path.display());
    }

    // The manpage root is the parent of `man1`.
    let root = dir.parent().unwrap_or(&dir);
    let indexed = process::run(std::process::Command::new("mandb").arg("-q").arg(root)).is_some();
    if !indexed {
        println!(
            "reedline-bash: could not run mandb -q. Try running `mandb -q {}` manually to refresh the index.",
            root.display()
        );
    }

    // Does the manpage show up with `man reedline`?
    let shown = process::run(std::process::Command::new("man").args(["-w", "reedline"]))
        .and_then(|path| std::fs::canonicalize(path.trim()).ok());
    let written = std::fs::canonicalize(dir.join("reedline.1")).ok();
    if shown.is_none() || shown != written {
        println!(
            "reedline-bash: `man reedline` does not show {} yet. Put {} first in MANPATH:\n  export MANPATH=\"{}:\"",
            dir.join("reedline.1").display(),
            root.display(),
            root.display()
        );
    }

    0
}

/// `$XDG_DATA_HOME/man/man1`, or its default under `$HOME`, which `man`
/// searches without configuration.
fn user_man1_dir() -> Option<std::path::PathBuf> {
    Some(config::data_dir()?.join("man/man1"))
}

static LONG_DOC: DocLines = DocLines([
    c"Reedline as bash's line editor.".as_ptr(),
    c"Loaded with: enable -f libreedline_bash.so reedline".as_ptr(),
    c"Subcommands: complete LINE, clear-cache, install-man. `reedline --help` says more.".as_ptr(),
    std::ptr::null(),
]);

/// The descriptor `enable -f` looks for by `<builtin>_struct`.
#[unsafe(no_mangle)]
pub static mut reedline_struct: symbols::Builtin = symbols::Builtin {
    name: c"reedline".as_ptr(),
    function: Some(call),
    flags: symbols::BUILTIN_ENABLED,
    long_doc: LONG_DOC.0.as_ptr(),
    short_doc: c"reedline [complete LINE | clear-cache | install-man [--dir DIR]]".as_ptr(),
    handle: std::ptr::null(),
};

/// Called by bash as the library loads. fails if returning 0
#[unsafe(no_mangle)]
pub extern "C" fn reedline_builtin_load(_name: *const c_char) -> c_int {
    guard(0, || {
        const OK: c_int = 1;
        const FAILED: c_int = 0;

        let interactive =
            unsafe { symbols::interactive_shell != 0 && symbols::no_line_editing == 0 };
        if !interactive {
            return OK;
        }

        let mut slot = match EDITOR.lock() {
            Ok(slot) => slot,
            Err(_) => return FAILED,
        };
        if slot.is_some() {
            return OK; // already loaded into bash
        }
        *slot = Some(Editor::new());

        unsafe { bash::input::install(get_char, unget_char) };
        OK
    })
}

/// Called by bash on `enable -d reedline`.
#[unsafe(no_mangle)]
pub extern "C" fn reedline_builtin_unload() {
    guard((), || {
        unsafe { bash::input::restore() };
        if let Ok(mut slot) = EDITOR.lock() {
            *slot = None;
        }
    })
}
