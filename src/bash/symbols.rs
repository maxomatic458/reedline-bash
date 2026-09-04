//! Bash's internal globals, functions and struct layouts.
#![allow(clippy::tabs_in_doc_comments)]
use std::os::raw::{c_char, c_int, c_ulong, c_void};

/// ```c
/// typedef int sh_cget_func_t (void);	/* sh_ivoidfunc_t */
/// ```
/// [`input.h:26`]
///
/// [`input.h:26`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/input.h?h=bash-5.3#n26
pub type GetFunc = unsafe extern "C" fn() -> c_int;

/// ```c
/// typedef int sh_cunget_func_t (int);	/* sh_intfunc_t */
/// ```
/// [`input.h:27`]
///
/// [`input.h:27`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/input.h?h=bash-5.3#n27
pub type UngetFunc = unsafe extern "C" fn(c_int) -> c_int;

/// ```c
/// enum stream_type {st_none, st_stdin, st_stream, st_string, st_bstream};
/// ```
/// [`input.h:29`]
///
/// [`input.h:29`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/input.h?h=bash-5.3#n29
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum StreamType {
    None = 0,
    Stdin = 1,
    Stream = 2,
    String = 3,
    BStream = 4,
}

/// ```c
/// typedef union {
///   FILE *file;
///   char *string;
///   int buffered_fd;
/// } INPUT_STREAM;
/// ```
/// [`input.h:72`]
///
/// [`input.h:72`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/input.h?h=bash-5.3#n72
#[repr(C)]
pub union InputStream {
    pub file: *mut c_void,
    pub string: *mut c_char,
    pub buffered_fd: c_int,
}

/// ```c
/// typedef struct {
///   enum stream_type type;
///   char *name;
///   INPUT_STREAM location;
///   sh_cget_func_t *getter;
///   sh_cunget_func_t *ungetter;
/// } BASH_INPUT;
/// ```
/// [`input.h:78`]
///
/// [`input.h:78`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/input.h?h=bash-5.3#n78
#[repr(C)]
pub struct BashInput {
    pub stream_type: StreamType,
    pub name: *mut c_char,
    pub location: InputStream,
    pub getter: Option<GetFunc>,
    pub ungetter: Option<UngetFunc>,
}

/// ```c
/// typedef struct stream_saver {
///   struct stream_saver *next;
///   BASH_INPUT bash_input;
///   int line;
///   BUFFERED_STREAM *bstream;
/// } STREAM_SAVER;
/// ```
/// [`parse.y:1872`]
///
/// Only `next` and `bash_input` are used.
///
/// [`parse.y:1872`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/parse.y?h=bash-5.3#n1872
#[repr(C)]
pub struct StreamSaver {
    pub next: *mut StreamSaver,
    pub bash_input: BashInput,
    pub line: c_int,
    pub bstream: *mut c_void,
}

/// ```c
/// /* The thing that we build the array of builtins out of. */
/// struct builtin {
///   char *name;			/* The name that the user types. */
///   sh_builtin_func_t *function;	/* The address of the invoked function. */
///   int flags;			/* One of the #defines above. */
///   char * const *long_doc;	/* NULL terminated array of strings. */
///   const char *short_doc;	/* Short version of documentation. */
///   char *handle;			/* for future use */
/// };
/// ```
/// [`builtins.h:53`]
///
/// [`builtins.h:53`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/builtins.h?h=bash-5.3#n53
#[repr(C)]
pub struct Builtin {
    pub name: *const c_char,
    pub function: Option<BuiltinFunc>,
    pub flags: c_int,
    pub long_doc: *const *const c_char,
    pub short_doc: *const c_char,
    pub handle: *const c_char,
}

/// The `sh_builtin_func_t` a [`Builtin`] holds. builtins are called with the
/// word list that followed their name.
pub type BuiltinFunc = extern "C" fn(*const WordList) -> c_int;

/// ```c
/// /* A structure which represents a word. */
/// typedef struct word_desc {
///   char *word;		/* Zero terminated string. */
///   int flags;		/* Flags associated with this word. */
/// } WORD_DESC;
/// ```
/// [`command.h:131`]
///
/// [`command.h:131`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/command.h?h=bash-5.3#n131
#[repr(C)]
pub struct WordDesc {
    pub word: *mut c_char,
    pub flags: c_int,
}

/// ```c
/// /* A linked list of words. */
/// typedef struct word_list {
///   struct word_list *next;
///   WORD_DESC *word;
/// } WORD_LIST;
/// ```
/// [`command.h:137`]
///
/// [`command.h:137`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/command.h?h=bash-5.3#n137
#[repr(C)]
pub struct WordList {
    pub next: *mut WordList,
    pub word: *mut WordDesc,
}

/// ```c
/// /* The structure used to store a history entry. */
/// typedef struct _hist_entry {
///   char *line;
///   char *timestamp;		/* char * rather than time_t for read/write */
///   histdata_t data;
/// } HIST_ENTRY;
/// ```
/// [`lib/readline/history.h:46`]
///
/// [`lib/readline/history.h:46`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/history.h?h=bash-5.3#n46
#[repr(C)]
pub struct HistoryEntry {
    pub line: *mut c_char,
    pub timestamp: *mut c_char,
    pub data: *mut c_void,
}

/// ```c
/// #define BUILTIN_ENABLED 0x01	/* This builtin is enabled. */
/// ```
/// [`builtins.h:41`]
///
/// [`builtins.h:41`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/builtins.h?h=bash-5.3#n41
pub const BUILTIN_ENABLED: c_int = 0x01;

pub const EOF: c_int = -1;

/// ```c
/// #define SEVAL_NOHIST	0x004
/// ```
/// [`builtins/common.h:46`] — keep the command out of the history.
///
/// [`builtins/common.h:46`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/builtins/common.h?h=bash-5.3#n46
pub const SEVAL_NOHIST: c_int = 0x004;

/// ```c
/// #define SEVAL_NOTIFY	0x800		/* want job notifications */
/// ```
/// [`builtins/common.h:55`]
///
/// [`builtins/common.h:55`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/builtins/common.h?h=bash-5.3#n55
pub const SEVAL_NOTIFY: c_int = 0x800;

// Safety for the statics below: the process is single-threaded -- nothing here
// spawns a thread and neither does reedline as configured -- so these are only
// ever touched from the thread bash calls into us on.

// SAFETY: all statics are only ever used inside of a single thread.
unsafe extern "C" {
    /// ```c
    /// extern BASH_INPUT bash_input;
    /// ```
    /// [`input.h:86`] — the stream bash's parser currently reads from.
    ///
    /// [`input.h:86`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/input.h?h=bash-5.3#n86
    pub static mut bash_input: BashInput;

    /// ```c
    /// STREAM_SAVER *stream_list = (STREAM_SAVER *)NULL;
    /// ```
    /// [`parse.y:1890`] — the stack of saved streams. During startup files this
    /// holds the stream bash will pop back to, which is the one to patch.
    ///
    /// [`parse.y:1890`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/parse.y?h=bash-5.3#n1890
    pub static mut stream_list: *mut StreamSaver;

    /// ```c
    /// extern int interactive, interactive_shell;
    /// ```
    /// [`shell.h:103`]
    ///
    /// [`shell.h:103`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/shell.h?h=bash-5.3#n103
    pub static mut interactive_shell: c_int;

    /// ```c
    /// extern int no_line_editing;
    /// ```
    /// [`shell.h:119`]
    ///
    /// [`shell.h:119`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/shell.h?h=bash-5.3#n119
    pub static mut no_line_editing: c_int;

    /// ```c
    /// extern pid_t original_pgrp, shell_pgrp, pipeline_pgrp;
    /// ```
    /// [`jobs.h:231`] — `pid_t` is `int` on the platforms this builds for.
    ///
    /// [`jobs.h:231`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/jobs.h?h=bash-5.3#n231
    pub static mut shell_pgrp: c_int;

    /// ```c
    /// extern int job_control;		/* set to 0 in nojobs.c */
    /// ```
    /// [`jobs.h:351`]
    ///
    /// [`jobs.h:351`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/jobs.h?h=bash-5.3#n351
    pub static mut job_control: c_int;

    /// ```c
    /// extern char *decode_prompt_string (char *, int);
    /// ```
    /// [`externs.h:131`] — expands a prompt string: `\u`, `\w`, `\$`, command
    /// substitution, `\[`/`\]`. The second argument is `is_prompt`, which bash
    /// passes as 1 from both of its own prompt call sites; it points
    /// `decoding_prompt` at the string so an embedded `${var@P}` resolves.
    ///
    /// [`externs.h:131`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/externs.h?h=bash-5.3#n131
    pub fn decode_prompt_string(string: *const c_char, is_prompt: c_int) -> *mut c_char;

    /// ```c
    /// extern void with_input_from_stdin (void);
    /// ```
    /// [`input.h:93`] — point the parser back at readline's reader. Acts only
    /// when `bash_input.type != st_stdin`, so restoring readline means clearing
    /// the type first. See `input::restore`.
    ///
    /// [`input.h:93`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/input.h?h=bash-5.3#n93
    pub fn with_input_from_stdin();

    /// ```c
    /// extern int give_terminal_to (pid_t, int);
    /// ```
    /// [`jobs.h:321`]
    ///
    /// [`jobs.h:321`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/jobs.h?h=bash-5.3#n321
    pub fn give_terminal_to(pgrp: c_int, force: c_int) -> c_int;

    /// ```c
    /// extern SHELL_VAR *find_variable (const char *);
    /// ```
    /// [`variables.h:286`] — the `SHELL_VAR` is opaque here; it is only ever
    /// handed straight back to [`get_variable_value`].
    ///
    /// [`variables.h:286`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/variables.h?h=bash-5.3#n286
    pub fn find_variable(name: *const c_char) -> *mut c_void;

    /// ```c
    /// extern char *get_variable_value (SHELL_VAR *);
    /// ```
    /// [`variables.h:329`]
    ///
    /// [`variables.h:329`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/variables.h?h=bash-5.3#n329
    pub fn get_variable_value(var: *mut c_void) -> *mut c_char;

    /// ```c
    /// extern PTR_T xmalloc (size_t);
    /// ```
    /// [`xmalloc.h:33`] — bash frees what it is given with its own allocator,
    /// so anything handed to it has to come from here.
    ///
    /// [`xmalloc.h:33`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/xmalloc.h?h=bash-5.3#n33
    pub fn xmalloc(size: usize) -> *mut c_void;

    /// ```c
    /// extern char *tilde_expand (const char *);
    /// ```
    /// [`lib/tilde/tilde.h:70`] — `~` and `~user` to a real path, using the
    /// same passwd lookup and `$HOME` reading bash does.
    ///
    /// [`lib/tilde/tilde.h:70`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/tilde/tilde.h?h=bash-5.3#n70
    pub fn tilde_expand(text: *const c_char) -> *mut c_char;

    /// ```c
    /// extern void xfree (void *);
    /// ```
    /// [`xmalloc.h:36`]
    ///
    /// [`xmalloc.h:36`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/xmalloc.h?h=bash-5.3#n36
    pub fn xfree(ptr: *mut c_void);

    /// ```c
    /// extern char **programmable_completions (const char *, const char *, int, int, int *);
    /// ```
    /// [`pcomplete.h:174`] — runs the compspec registered for `cmd`. `foundp`
    /// reports which `-o` options it carried, and is zero when none existed.
    ///
    /// `start` and `end` bound the **command**, not the word: bash slices
    /// `COMP_LINE` out of them. See `words::command_bounds`.
    ///
    /// [`pcomplete.h:174`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/pcomplete.h?h=bash-5.3#n174
    pub fn programmable_completions(
        cmd: *const c_char,
        word: *const c_char,
        start: c_int,
        end: c_int,
        foundp: *mut c_int,
    ) -> *mut *mut c_char;

    /// ```c
    /// extern void pcomp_set_readline_variables (int, int);
    /// ```
    /// [`pcomplete.h:176`] — apply a compspec's `-o` options to readline's
    /// globals.
    ///
    /// [`pcomplete.h:176`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/pcomplete.h?h=bash-5.3#n176
    pub fn pcomp_set_readline_variables(flags: c_int, nval: c_int);

    /// ```c
    /// extern char **bash_default_completion (const char *, int, int, int, int);
    /// ```
    /// [`bashline.h:56`] — what readline falls back to when no compspec
    /// applies: command names, shell variables, `~user`, hostnames.
    ///
    /// [`bashline.h:56`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/bashline.h?h=bash-5.3#n56
    pub fn bash_default_completion(
        text: *const c_char,
        start: c_int,
        end: c_int,
        quote_char: c_int,
        compflags: c_int,
    ) -> *mut *mut c_char;

    /// ```c
    /// extern void strvec_dispose (char **);
    /// ```
    /// [`externs.h:432`] — free a `char **` and everything in it.
    ///
    /// [`externs.h:432`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/externs.h?h=bash-5.3#n432
    pub fn strvec_dispose(array: *mut *mut c_char);

    /// ```c
    /// extern HIST_ENTRY **history_list (void);
    /// ```
    /// [`lib/readline/history.h:136`] — the shell's history, oldest first,
    /// null-terminated. Authoritative in a way `HISTFILE` is not: bash appends
    /// every line we hand it, so this is current mid-session.
    ///
    /// [`lib/readline/history.h:136`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/history.h?h=bash-5.3#n136
    pub fn history_list() -> *mut *mut HistoryEntry;

    /// ```c
    /// extern int parse_and_execute (char *, const char *, int);
    /// ```
    /// [`builtins/common.h:135`] — what `bind -x` runs its command with. Takes
    /// the string, and frees it unless `SEVAL_NOFREE`.
    ///
    /// [`builtins/common.h:135`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/builtins/common.h?h=bash-5.3#n135
    pub fn parse_and_execute(string: *mut c_char, from_file: *const c_char, flags: c_int) -> c_int;

    /// ```c
    /// extern void save_parser_state (sh_parser_state_t *);
    /// ```
    /// [`externs.h:396`] — the parser is mid-line while it waits on us, so
    /// running a command underneath it has to leave that state alone.
    ///
    /// [`externs.h:396`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/externs.h?h=bash-5.3#n396
    pub fn save_parser_state(state: *mut c_void) -> *mut c_void;

    /// ```c
    /// extern void restore_parser_state (sh_parser_state_t *);
    /// ```
    /// [`externs.h:397`]
    ///
    /// [`externs.h:397`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/externs.h?h=bash-5.3#n397
    pub fn restore_parser_state(state: *mut c_void);

    /// ```c
    /// extern int history_length;
    /// ```
    /// [`lib/readline/history.h:253`] — how many entries [`history_list`] has,
    /// which is otherwise only findable by walking to its terminator.
    ///
    /// [`lib/readline/history.h:253`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/history.h?h=bash-5.3#n253
    pub static mut history_length: c_int;

    /// ```c
    /// extern char **rl_completion_matches (const char *, rl_compentry_func_t *);
    /// ```
    /// [`lib/readline/readline.h:495`] — run a generator until it stops
    /// producing, and package the results readline's way: element 0 is the
    /// longest common prefix when two or more matches follow.
    ///
    /// [`lib/readline/readline.h:495`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n495
    pub fn rl_completion_matches(
        text: *const c_char,
        generator: unsafe extern "C" fn(*const c_char, c_int) -> *mut c_char,
    ) -> *mut *mut c_char;

    /// ```c
    /// extern char *rl_filename_completion_function (const char *, int);
    /// ```
    /// [`lib/readline/readline.h:497`] — the generator behind plain filename
    /// completion.
    ///
    /// [`lib/readline/readline.h:497`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n497
    pub fn rl_filename_completion_function(text: *const c_char, state: c_int) -> *mut c_char;

    // Readline's state. It is not running, so we maintain these ourselves --
    // `programmable_completions` copies the first two into `pcomp_line` and
    // `pcomp_ind` on entry, and that is where `COMP_LINE` and `COMP_POINT` come
    // from. All from lib/readline/readline.h at bash-5.3.

    /// `extern char *rl_line_buffer;` — [`readline.h:554`]
    ///
    /// [`readline.h:554`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n554
    pub static mut rl_line_buffer: *mut c_char;

    /// `extern int rl_point;` — [`readline.h:557`]
    ///
    /// [`readline.h:557`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n557
    pub static mut rl_point: c_int;

    /// `extern int rl_end;` — [`readline.h:558`]
    ///
    /// [`readline.h:558`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n558
    pub static mut rl_end: c_int;

    /// `extern unsigned long rl_readline_state;` — [`readline.h:531`], the flag
    /// word `RL_STATE_COMPLETING` lives in.
    ///
    /// [`readline.h:531`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n531
    pub static mut rl_readline_state: c_ulong;

    /// `extern int rl_completion_append_character;` — [`readline.h:855`]
    ///
    /// [`readline.h:855`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n855
    pub static mut rl_completion_append_character: c_int;

    /// `extern int rl_completion_suppress_append;` — [`readline.h:859`]
    ///
    /// [`readline.h:859`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n859
    pub static mut rl_completion_suppress_append: c_int;

    /// `extern int rl_completion_quote_character;` — [`readline.h:863`]
    ///
    /// [`readline.h:863`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n863
    pub static mut rl_completion_quote_character: c_int;

    /// `extern int rl_completion_found_quote;` — [`readline.h:867`]
    ///
    /// [`readline.h:867`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n867
    pub static mut rl_completion_found_quote: c_int;

    /// `extern int rl_filename_completion_desired;` — [`readline.h:805`]
    ///
    /// [`readline.h:805`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n805
    pub static mut rl_filename_completion_desired: c_int;

    /// `extern int rl_filename_quoting_desired;` — [`readline.h:812`]
    ///
    /// [`readline.h:812`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n812
    pub static mut rl_filename_quoting_desired: c_int;

    /// `extern int rl_full_quoting_desired;` — [`readline.h:815`]. The other
    /// half of readline's `QUOTING_DESIRED`, which decides whether a match is
    /// quoted at all.
    ///
    /// [`readline.h:815`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n815
    pub static mut rl_full_quoting_desired: c_int;

    /// `extern int rl_sort_completion_matches;` — [`readline.h:875`]
    ///
    /// [`readline.h:875`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n875
    pub static mut rl_sort_completion_matches: c_int;

    /// `extern const char *rl_completer_word_break_characters;` —
    /// [`readline.h:711`]. Bash keeps this in sync with `$COMP_WORDBREAKS`
    /// while readline is running; it is null when readline never started.
    ///
    /// [`readline.h:711`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n711
    pub static mut rl_completer_word_break_characters: *const c_char;

    /// `extern int rl_attempted_completion_over;` — [`readline.h:838`], set by
    /// a compspec that wants no fallback attempted.
    ///
    /// [`readline.h:838`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n838
    pub static mut rl_attempted_completion_over: c_int;

    /// ```c
    /// extern void rl_replace_line (const char *, int);
    /// ```
    /// [`lib/readline/readline.h:425`] — put text into `rl_line_buffer`, growing
    /// it through readline's own `rl_line_buffer_len` bookkeeping. Swapping the
    /// pointer for a smaller allocation leaves that length stale, and the next
    /// `read -e` writes past the end.
    ///
    /// [`lib/readline/readline.h:425`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/lib/readline/readline.h?h=bash-5.3#n425
    pub fn rl_replace_line(text: *const c_char, clear_undo: c_int);

    /// ```c
    /// extern void bashline_set_filename_hooks (void);
    /// ```
    /// [`bashline.h:58`] — the hooks readline's filename completion calls back
    /// into bash through: expanding `$VAR` and `~` in a directory name, and
    /// `direxpand`. Bash installs them at the top of `attempt_shell_completion`.
    ///
    /// [`bashline.h:58`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/bashline.h?h=bash-5.3#n58
    pub fn bashline_set_filename_hooks();

    /// ```c
    /// extern void set_exit_status (int);
    /// ```
    /// [`externs.h:85`] — `$?` and `PIPESTATUS`.
    ///
    /// [`externs.h:85`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/externs.h?h=bash-5.3#n85
    pub fn set_exit_status(status: c_int);

    /// ```c
    /// extern int prog_completion_enabled;
    /// ```
    /// [`pcomplete.h:118`] — `shopt progcomp`.
    ///
    /// [`pcomplete.h:118`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/pcomplete.h?h=bash-5.3#n118
    pub static mut prog_completion_enabled: c_int;

    /// ```c
    /// int force_fignore = 1;
    /// ```
    /// [`bashline.c:292`] — `shopt force_fignore`: when off, a `FIGNORE` that
    /// would leave no match is not applied.
    ///
    /// [`bashline.c:292`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/bashline.c?h=bash-5.3#n292
    pub static mut force_fignore: c_int;

    /// ```c
    /// char *current_readline_prompt = (char *)NULL;
    /// ```
    /// [`parse.y:1651`] — the prompt bash decoded for the line it is about to
    /// read: `PS1`, or `PS2` when the parser wants a continuation. `prompt_again`
    /// fills it before every read from an `st_stdin` stream, ours included, so
    /// decoding `PS1` again would run its command substitutions twice.
    ///
    /// [`parse.y:1651`]: https://cgit.git.savannah.gnu.org/cgit/bash.git/tree/parse.y?h=bash-5.3#n1651
    pub static mut current_readline_prompt: *mut c_char;
}

/// The prompt bash decoded for the line it is asking for, or `None` before it
/// has decoded one.
///
/// # Safety
/// Reads bash's globals; must run on the thread bash called into us on.
pub unsafe fn current_prompt() -> Option<String> {
    let prompt = unsafe { current_readline_prompt };
    if prompt.is_null() {
        return None;
    }
    Some(
        unsafe { std::ffi::CStr::from_ptr(prompt) }
            .to_string_lossy()
            .into_owned(),
    )
}

/// Copy `text` into an allocation bash is allowed to free.
///
/// Bash frees `bash_input.name` itself, so a Rust-owned `CString` handed over
/// there would be freed by the wrong allocator.
///
/// # Safety
/// Calls bash's allocator; must run on the thread bash called into us on.
pub unsafe fn bash_strdup(text: &str) -> *mut c_char {
    let bytes = text.as_bytes();
    let buffer = unsafe { xmalloc(bytes.len() + 1) } as *mut c_char;
    if buffer.is_null() {
        return buffer;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer as *mut u8, bytes.len());
        *buffer.add(bytes.len()) = 0;
    }
    buffer
}

/// Read a shell variable, or `None` when it is unset.
///
/// # Safety
/// Calls into bash. must run on the thread bash called into us on.
pub unsafe fn shell_variable(name: &str) -> Option<String> {
    let c_name = std::ffi::CString::new(name).ok()?;
    let var = unsafe { find_variable(c_name.as_ptr()) };
    if var.is_null() {
        return None;
    }
    let value = unsafe { get_variable_value(var) };
    if value.is_null() {
        return None;
    }
    Some(
        unsafe { std::ffi::CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned(),
    )
}

/// How many commands the shell is holding.
///
/// # Safety
/// Calls into bash. must run on the thread bash called into us on.
pub unsafe fn history_len() -> usize {
    unsafe { usize::try_from(history_length).unwrap_or(0) }
}

/// The command at `index`, counting from the oldest.
///
/// # Safety
/// Calls into bash. Must run on the thread bash called into us on. `index` must
/// be below whatever [`history_len`] last returned.
pub unsafe fn history_line<'a>(index: usize) -> Option<&'a [u8]> {
    unsafe {
        let list = history_list();
        if list.is_null() {
            return None;
        }
        // An off-by-one lands on the terminator, so it comes back empty.
        let entry = *list.add(index);
        if entry.is_null() || (*entry).line.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr((*entry).line).to_bytes())
    }
}

/// The arguments a builtin was called with.
///
/// # Safety
/// `words` must be the list bash passed to a builtin, or null.
pub unsafe fn word_list(words: *const WordList) -> Vec<String> {
    let mut args = Vec::new();
    let mut node = words;
    unsafe {
        while !node.is_null() {
            let desc = (*node).word;
            if !desc.is_null() && !(*desc).word.is_null() {
                args.push(
                    std::ffi::CStr::from_ptr((*desc).word)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            node = (*node).next;
        }
    }
    args
}

/// Expand the prompt in the same exact way bash does.
///
/// # Safety
/// Calls into bash; must run on the thread bash called into us on.
pub unsafe fn expand_prompt(raw: &str) -> Option<String> {
    let c_raw = std::ffi::CString::new(raw).ok()?;
    // bash sets `is_prompt` to 1
    let decoded = unsafe { decode_prompt_string(c_raw.as_ptr(), 1) };
    if decoded.is_null() {
        return None;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(decoded) }
        .to_string_lossy()
        .into_owned();
    unsafe { xfree(decoded as *mut c_void) };
    Some(text)
}

/// Expand a leading `~`, or `None` if bash could not.
///
/// # Safety
/// Calls into bash; must run on the thread bash called into us on.
pub unsafe fn expand_tilde(path: &str) -> Option<String> {
    let c_path = std::ffi::CString::new(path).ok()?;
    let expanded = unsafe { tilde_expand(c_path.as_ptr()) };
    if expanded.is_null() {
        return None;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(expanded) }
        .to_string_lossy()
        .into_owned();
    unsafe { xfree(expanded as *mut c_void) };
    Some(text)
}

/// Room for a `sh_parser_state_t`
#[repr(align(16))]
struct ParserState([u8; 4096]);

/// Run `command` the way bash's `bind -x` does
///
/// An `exit` in there makes bash `longjmp` to its top level, straight over
/// the Rust frames between here and `get_char`. Bash does the same to itself
/// under `bind -x`; the shell is ending either way.
///
/// # Safety
/// Calls into bash; must run on the thread bash called into us on.
pub unsafe fn run_host_command(command: &str) -> c_int {
    unsafe {
        let owned = bash_strdup(command);
        if owned.is_null() {
            return 1;
        }
        let mut state = ParserState([0; 4096]);
        let saved = state.0.as_mut_ptr().cast();
        save_parser_state(saved);

        // Mirrors `bash_execute_unix_command`.
        let flags = if interactive_shell != 0 {
            SEVAL_NOTIFY | SEVAL_NOHIST
        } else {
            SEVAL_NOHIST
        };
        let status = parse_and_execute(owned, c"reedline".as_ptr(), flags);

        restore_parser_state(saved);
        status
    }
}
