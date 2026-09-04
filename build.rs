//! Generates the `reedline` builtin's manual pages from clap.
use std::{env, fs, io, path::PathBuf};

include!("src/cli.rs");

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=build.rs");

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let man = out.join("man");
    fs::create_dir_all(&man)?;
    clap_mangen::generate_to(command(), &man)?;

    // A table of the pages for `include_bytes!`, by file name.
    let mut names: Vec<String> = fs::read_dir(&man)?
        .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<_>>()?;
    names.sort();
    let mut table = String::from(
        "/// Each manual page, by file name.\npub const PAGES: &[(&str, &[u8])] = &[\n",
    );
    for name in names {
        table.push_str(&format!(
            "    ({name:?}, include_bytes!({:?})),\n",
            man.join(&name).display()
        ));
    }
    table.push_str("];\n");
    fs::write(out.join("manpages.rs"), table)
}
