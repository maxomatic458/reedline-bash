# reedline-bash

A bash plugin that replaces bash's line editor, readline, with
[reedline](https://github.com/nushell/reedline) — the line editor powering
nushell.

you keep your existing bash prompt, completions and history.

## Installation

```bash
cargo build --release
```

Enable `reedline-bash` in your current bash session:

```bash
enable -f $PWD/target/release/libreedline_bash.so reedline
```

To enable it permanently, add the same line to your `~/.bashrc`:

```bash
echo "enable -f path/to/libreedline_bash.so reedline" >> ~/.bashrc
```

## Configuration

You can configure reedline-bash by creating a `~/.config/reedline-bash/config.toml` file. 
See `config.example.toml` for the defaults.

An edit applies from the next prompt, so there is no need to restart the shell.
Note that `~/.inputrc` is not read: it configures readline, which no longer
sees your keys, so keybindings belong in `config.toml`.

## How it works

Readline is linked into the bash binary, so it cannot be swapped out from
outside. But bash's parser never calls readline directly — it reads characters
through a function pointer, and that pointer can be replaced.

```
                         bash process
  ┌──────────────────────────────────────────────────────────┐
  │  parser ── wants a character ──▶ get_char()   ← ours      │
  │                                      │                    │
  │                                      ▼                    │
  │                          Reedline::read_line()            │
  │                            prompt, editing, menus         │
  │                                      │                    │
  │  parser ◀── one byte at a time ── the finished line       │
  └──────────────────────────────────────────────────────────┘
```

1. `enable -f lib.so reedline` is bash's loadable-builtin mechanism: it
   `dlopen`s the object into bash's *own process*. Bash is linked with
   `-rdynamic`, so the object can reach bash's internals.
2. We point `bash_input.getter` at our `get_char`, and the parser starts
   reading through it instead of readline.
3. The parser asks for one character at a time. When our buffer runs dry we
   call `Reedline::read_line()`.
4. Reedline draws the prompt and handles the editing, then returns the line.
5. We hand it back byte by byte, and bash runs it as it always did.

Living inside the bash process is what makes the rest come for free. The prompt
is your `PS1`, expanded by bash's own `decode_prompt_string()`. Completion calls
the same functions readline would, so `complete -F` and bash-completion work
untouched. History is bash's live list rather than a copy of it.

## References and sources

- [flyline](https://github.com/HalFrgrd/flyline): copied over the main logic for
  replacing bash's readline. reedline-bash includes some of its grammar definitions
  directly.
- [reedline](https://github.com/nushell/reedline): the line editor we replace readline with
- [flash](https://github.com/HalFrgrd/flash): the shell lexer used
- [bash](https://git.savannah.gnu.org/cgit/bash.git): struct layouts and symbol
  names are checked against 5.3

## License

This project's code is licensed under [MIT](./LICENSE.md). 

Compiled binaries are licensed under [GPLv3](./LICENSE-GPL).
