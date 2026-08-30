# reedline-bash

![Syntax highlighting, suggestions from history, completion menus over bash's
own completions, abbreviation expansions, and multiline edits](reedline-bash.gif)

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
  native bash          reedline-bash
  -----------          -------------
  parser               parser
  |                    |
  v                    v
  readline             get_char  <- We inject this by replacing
  |                    |            the `bash_input.getter` function
  |                    v            pointer.
  |                    reedline
  |                    |
  v                    v
  your terminal        your terminal

```

1. We load reedline-bash into the bash process with: `enable -f reedline-bash.so reedline`
2. Once loaded `bash_input.getter` points to our `get_char`. Bash will read
   through it instead of readline.
3. Reedline draws the prompt and handles the editing, then returns the line.
4. We hand it back byte by byte, and bash runs normally.

reedline-bash still uses your native bash completions and history, for inline suggestions and the completion menus provided by reedline.

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
