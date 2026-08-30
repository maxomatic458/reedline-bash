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
//! Reedline uses bash's native history ([`history::from_bash`]) and completions
//! ([`bash::complete::candidates`]).

pub mod bash;
mod completer;
mod config;
mod editor;
mod grammar;
mod highlighter;
pub mod history;
mod keys;
mod prompt;
mod style;
mod validator;
mod words;

use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

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
    guard(symbols::EOF, || {
        match EDITOR.lock() {
            Ok(mut editor) => match editor.as_mut() {
                Some(editor) => editor.feeder.next_char(),
                None => symbols::EOF,
            },
            // An earlier call panicked
            Err(_) => symbols::EOF,
        }
    })
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

/// The `reedline` command itself.
///
/// `--complete LINE` prints what the completer would offer for LINE, one
/// suggestion per line.
extern "C" fn call(words: *const symbols::WordList) -> c_int {
    guard(1, || {
        let args = unsafe { symbols::word_list(words) };
        match args.first().map(String::as_str) {
            Some("--complete") => {
                let line = args.get(1).cloned().unwrap_or_default();
                let point = line.len();
                let start =
                    words::word_start_with_breaks(&line, point, &bash::complete::word_breaks());
                for candidate in unsafe { bash::complete::candidates(&line, start, point) }.matches
                {
                    println!("{candidate}");
                }
                0
            }
            _ => {
                println!("reedline-bash {} (loaded)", env!("CARGO_PKG_VERSION"));
                0
            }
        }
    })
}

static LONG_DOC: DocLines = DocLines([
    c"Reedline as bash's line editor.".as_ptr(),
    c"Loaded with: enable -f libreedline_bash.so reedline".as_ptr(),
    std::ptr::null(),
]);

/// The descriptor `enable -f` looks for by `<builtin>_struct`.
#[unsafe(no_mangle)]
pub static mut reedline_struct: symbols::Builtin = symbols::Builtin {
    name: c"reedline".as_ptr(),
    function: Some(call),
    flags: symbols::BUILTIN_ENABLED,
    long_doc: LONG_DOC.0.as_ptr(),
    short_doc: c"reedline [option]".as_ptr(),
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
