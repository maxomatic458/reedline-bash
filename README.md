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
echo "enable -f $PWD/target/release/libreedline_bash.so reedline" >> ~/.bashrc
```

## Configuration

You can configure reedline-bash by creating a `~/.config/reedline-bash/config.toml` file. 
See `config.example.toml` for the defaults.

TODO

## How it works

TODO

## References and sources

- [flyline](https://github.com/HalFrgrd/flyline): copied over the main logic for
  replacing bash's readline. reedline-bash includes some of its grammar definitions
  directly.
- [reedline](https://github.com/nushell/reedline): the line editor we replace readline with
- [flash](https://github.com/HalFrgrd/flash): the shell lexer used in `src/grammar/` 
- [bash](https://git.savannah.gnu.org/cgit/bash.git): struct layouts and symbol
  names are checked against 5.3

## License

This project's code is licensed under [MIT](./LICENSE.md). 

Compiled binaries are licensed under [GPLv3](./LICENSE-GPL).
