//! Taking over bash's input.
//!
//! Bash reads the input via "bash_input.getter". This is replaced bypassing
//! regular readline.
use std::os::raw::c_int;

use super::symbols::{self, BashInput, StreamType};

/// Bash reads single characters, but reedline returns an entire line at once.
///
/// Therefore we use [`LineFeeder`] to buffer the line and feed it into bash byte by byte.
pub struct LineFeeder<F> {
    buffer: Vec<u8>,
    position: usize,
    /// Draws the prompt and blocks. Returns the next line without terminator,
    /// or `None` for an end of input.
    fetch: F,
}

impl<F: FnMut() -> Option<String>> LineFeeder<F> {
    pub fn new(fetch: F) -> Self {
        LineFeeder {
            buffer: Vec::new(),
            position: 0,
            fetch,
        }
    }

    /// The next character fed into bash, or [`symbols::EOF`].
    pub fn next_char(&mut self) -> c_int {
        if self.position >= self.buffer.len() {
            match (self.fetch)() {
                Some(line) => {
                    // We need to add the newline
                    self.buffer = line.into_bytes();
                    self.buffer.push(b'\n');
                    self.position = 0;
                }
                None => return symbols::EOF,
            }
        }
        let byte = self.buffer[self.position];
        self.position += 1;
        c_int::from(byte)
    }

    /// Put a character back, to be handed out again by the next `next_char`.
    pub fn unget(&mut self, c: c_int) -> c_int {
        if self.position > 0 {
            self.position -= 1;
            // Bash ungets what it just read, but it is allowed to unget
            // anything, and then that is what the parser must see next.
            if let Ok(byte) = u8::try_from(c) {
                self.buffer[self.position] = byte;
            }
        }
        c
    }
}

/// Point a `BASH_INPUT` at our getter.
///
/// # Safety
/// `target` must be a live `BASH_INPUT`.
pub unsafe fn redirect(
    target: *mut BashInput,
    getter: symbols::GetFunc,
    ungetter: symbols::UngetFunc,
) {
    unsafe {
        let previous_name = (*target).name;

        (*target).stream_type = StreamType::Stdin;
        (*target).name = symbols::bash_strdup("reedline");
        (*target).getter = Some(getter);
        (*target).ungetter = Some(ungetter);

        // Bash allocated the old name and expects to free it
        if !previous_name.is_null() {
            symbols::xfree(previous_name as *mut std::ffi::c_void);
        }
    }
}

/// The stream bash reads the terminal through.
///
/// # Safety
/// Reads bash's globals; must run on the thread bash called into us on.
unsafe fn terminal_stream() -> *mut BashInput {
    unsafe {
        let mut saver = symbols::stream_list;
        if saver.is_null() {
            return &raw mut symbols::bash_input;
        }
        while !(*saver).next.is_null() {
            saver = (*saver).next;
        }
        &raw mut (*saver).bash_input
    }
}

/// Install the reader into bash.
///
/// # Safety
/// Must run from `<name>_builtin_load`.
pub unsafe fn install(getter: symbols::GetFunc, ungetter: symbols::UngetFunc) {
    unsafe { redirect(terminal_stream(), getter, ungetter) }
}

/// Hand the input stream back to readline.
///
/// # Safety
/// Must run from `<name>_builtin_unload`
pub unsafe fn restore() {
    unsafe {
        let ours = terminal_stream();
        let current = &raw mut symbols::bash_input;

        // `with_input_from_stdin` installs readline's reader into `bash_input`,
        (*ours).stream_type = StreamType::None;
        if ours == current {
            symbols::with_input_from_stdin();
            return;
        }

        // A file is being read through `bash_input`
        let in_flight = std::ptr::read(current);
        std::ptr::write(
            current,
            BashInput {
                stream_type: StreamType::None,
                name: std::ptr::null_mut(),
                location: symbols::InputStream { buffered_fd: 0 },
                getter: None,
                ungetter: None,
            },
        );
        symbols::with_input_from_stdin();
        let readline = std::ptr::read(current);
        std::ptr::write(current, in_flight);

        if !(*ours).name.is_null() {
            symbols::xfree((*ours).name as *mut std::ffi::c_void);
        }
        std::ptr::write(ours, readline);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feeder(lines: Vec<&str>) -> LineFeeder<impl FnMut() -> Option<String>> {
        let mut queue: Vec<String> = lines.into_iter().rev().map(String::from).collect();
        LineFeeder::new(move || queue.pop())
    }

    /// Every byte bash's parser receives, until end of input.
    fn drain_bytes(feeder: &mut LineFeeder<impl FnMut() -> Option<String>>) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let c = feeder.next_char();
            if c == symbols::EOF {
                return out;
            }
            out.push(c as u8);
        }
    }

    fn drain(feeder: &mut LineFeeder<impl FnMut() -> Option<String>>) -> String {
        String::from_utf8(drain_bytes(feeder)).expect("the feeder must not split a character")
    }

    #[test]
    fn a_line_is_handed_over_one_character_at_a_time() {
        let mut feeder = feeder(vec!["echo hi"]);
        assert_eq!(drain(&mut feeder), "echo hi\n");
    }

    #[test]
    fn the_terminator_is_added_because_the_editor_does_not_return_one() {
        // The parser needs it to know the command is complete.
        let mut feeder = feeder(vec!["a", "b"]);
        assert_eq!(drain(&mut feeder), "a\nb\n");
    }

    #[test]
    fn a_multiline_command_keeps_its_internal_newlines() {
        let mut feeder = feeder(vec!["for i in 1 2; do\necho $i\ndone"]);
        assert_eq!(drain(&mut feeder), "for i in 1 2; do\necho $i\ndone\n");
    }

    #[test]
    fn an_empty_line_is_still_a_newline() {
        // Pressing Enter on an empty line has to reach the parser, or bash never
        // prints another prompt.
        let mut feeder = feeder(vec!["", ""]);
        assert_eq!(drain(&mut feeder), "\n\n");
    }

    #[test]
    fn eof_is_reported_once_and_the_fetcher_is_asked_again() {
        // Bash decides what an EOF means
        let mut answers: Vec<Option<String>> = vec![Some("x".into()), None, Some("y".into())];
        answers.reverse();
        let mut feeder = LineFeeder::new(move || answers.pop().flatten());
        // The first drain stops at the EOF in the middle
        assert_eq!(drain(&mut feeder), "x\n");
        assert_eq!(drain(&mut feeder), "y\n");
    }

    #[test]
    fn a_character_put_back_is_handed_out_again() {
        let mut feeder = feeder(vec!["ab"]);
        assert_eq!(feeder.next_char(), i32::from(b'a'));
        assert_eq!(feeder.next_char(), i32::from(b'b'));
        feeder.unget(i32::from(b'b'));
        assert_eq!(feeder.next_char(), i32::from(b'b'));
        assert_eq!(feeder.next_char(), i32::from(b'\n'));
    }

    #[test]
    fn unget_at_the_start_of_a_line_does_not_underflow() {
        let mut feeder = feeder(vec!["a"]);
        feeder.unget(i32::from(b'z'));
        assert_eq!(feeder.next_char(), i32::from(b'a'));
    }

    #[test]
    fn utf8_is_handed_over_a_byte_at_a_time() {
        // Bash's parser is byte-oriented; multi-byte characters must not be
        // truncated to one `int`.
        let mut feeder = feeder(vec!["echo ä"]);
        assert_eq!(drain_bytes(&mut feeder), "echo ä\n".as_bytes());
    }

    #[test]
    fn a_different_character_put_back_is_the_one_handed_out() {
        let mut feeder = feeder(vec!["ab"]);
        assert_eq!(feeder.next_char(), i32::from(b'a'));
        feeder.unget(i32::from(b'z'));
        assert_eq!(feeder.next_char(), i32::from(b'z'));
        assert_eq!(feeder.next_char(), i32::from(b'b'));
    }
}
