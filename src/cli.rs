// The `reedline` builtin's command line

use clap::{Arg, Command, value_parser};

const EXTRA: &str = "\
Configuration:
  ~/.config/reedline-bash/config.toml, or $XDG_CONFIG_HOME/reedline-bash/config.toml,
  or the file $REEDLINE_BASH_CONFIG names. Edits apply from the next prompt.

Cache:
  Parsed manpages are kept under $XDG_CACHE_HOME/reedline-bash/man
  (~/.cache/reedline-bash/man), one gzip compressed file per page.

Loading:
  enable -f /path/to/libreedline_bash.so reedline   load it into this shell
  enable -d reedline                                hand the shell back to readline";

pub fn command() -> Command {
    Command::new("reedline")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Reedline as bash's line editor")
        .long_about(
            "Once loaded with `enable -f`, bash reads its lines through reedline instead of \
             readline",
        )
        .after_long_help(EXTRA)
        .subcommand(
            Command::new("complete")
                .about("Print the completions offered at the end of LINE")
                .arg(
                    Arg::new("line")
                        .value_name("LINE")
                        .required(true)
                        .help("The command line so far"),
                ),
        )
        .subcommand(
            Command::new("clear-cache").about("Delete the parsed manual pages kept on disk"),
        )
        .subcommand(
            Command::new("install-man")
                .about("Install the manual pages for this command")
                .arg(
                    Arg::new("dir")
                        .long("dir")
                        .value_name("DIR")
                        .value_parser(value_parser!(std::path::PathBuf))
                        .help("The man1 directory to write into"),
                ),
        )
}
