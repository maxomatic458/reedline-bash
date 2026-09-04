//! Descriptions for completion suggestions: command summaries from `whatis`
//! and `help -d` (bash builtins), options and subcommands parsed from man pages by [`man`].

mod man;

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use crate::process::run;

use man::{PageTable, parse_help_d, parse_options, parse_subcommands, parse_whatis};

pub trait Describer: Send {
    /// What the command `name` does.
    /// e.g "git - the stupid content tracker"
    fn command(&mut self, name: &str) -> Option<String>;

    /// What the subcommand `name` of `command` does, if it is one.
    /// e.g "git add - Add file contents to the index"
    fn subcommand(&mut self, command: &[String], name: &str) -> Option<String>;

    /// What `option` does when given to `command`.
    /// e.g "git --version - Prints the Git suite version..."
    fn option(&mut self, command: &[String], option: &str) -> Option<String>;
}

/// For `descriptions = false`, and tests.
pub struct NoDescriptions;

impl Describer for NoDescriptions {
    fn command(&mut self, _name: &str) -> Option<String> {
        None
    }

    fn subcommand(&mut self, _command: &[String], _name: &str) -> Option<String> {
        None
    }

    fn option(&mut self, _command: &[String], _option: &str) -> Option<String> {
        None
    }
}

/// Where parsed pages are stored: `$XDG_CACHE_HOME/reedline-bash/man`.
pub fn cache_dir() -> Option<PathBuf> {
    crate::config::cache_dir().map(|dir| dir.join("man"))
}

/// Delete the on-disk cache.
///
/// # Returns
/// - `Ok((files, bytes))` the number of files and their total size deleted.
pub fn clear_cache_dir(dir: &Path) -> std::io::Result<(usize, u64)> {
    let mut files = 0;
    let mut bytes = 0;
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok((0, 0)),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        bytes += entry.metadata()?.len();
        std::fs::remove_file(entry.path())?;
        files += 1;
    }
    Ok((files, bytes))
}

/// Descriptions from `whatis`, `man` and bash's `help`.
pub struct ManDescriber {
    summaries: Option<Summaries>,
    /// Per page: its options and the subcommands it lists.
    pages: HashMap<String, PageTable>,
    /// Where parsed option tables are kept between sessions.
    cache_dir: Option<PathBuf>,
}

/// One line per command, from the manual index and the shell.
struct Summaries {
    by_name: HashMap<String, String>,
    builtins: HashSet<String>,
    /// The manual index as it was when this was read.
    index_stamp: IndexStamp,
}

/// man-db's index of the system manual.
const MAN_INDEX: &str = "/var/cache/man/index.db";

/// A compressed page bigger than this is not parsed.
const MAX_PAGE_BYTES: u64 = 1024 * 1024; // 1 MiB

impl ManDescriber {
    pub fn new(cache_dir: Option<PathBuf>) -> Self {
        ManDescriber {
            summaries: None,
            pages: HashMap::new(),
            cache_dir,
        }
    }

    fn summaries(&mut self) -> &Summaries {
        let stamp = index_stamp();
        if self
            .summaries
            .as_ref()
            .is_none_or(|known| known.index_stamp != stamp)
        {
            let mut by_name = HashMap::new();
            let mut builtins = HashSet::new();
            // Bash's own builtins.
            if let Some(text) = run(Command::new("bash").args(["-c", "help -d '*'"])) {
                for (name, summary) in parse_help_d(&text) {
                    builtins.insert(name.clone());
                    by_name.insert(name, summary);
                }
            }
            if let Some(text) = run(Command::new("whatis").args(["-w", "*"])) {
                for (name, summary) in parse_whatis(&text) {
                    by_name.entry(name).or_insert(summary);
                }
            }
            self.summaries = Some(Summaries {
                by_name,
                builtins,
                index_stamp: stamp,
            });
        }
        self.summaries.as_ref().expect("filled just above")
    }

    /// The page that documents `command`: e.g `git-add` for `git add`.
    fn page_for(&mut self, command: &[String]) -> Option<String> {
        let names = page_names(command);
        let (first, rest) = names.split_first()?;
        let known = &self.summaries().by_name;
        let mut page = first.to_string();
        let mut depth = 0;
        for word in rest {
            let deeper = format!("{page}-{word}");
            if known.contains_key(&deeper) {
                page = deeper;
                depth += 1;
                if depth == 2 {
                    break;
                }
            } else if depth > 0 {
                // after the subcommand, the words are the arguments.
                break;
            }
        }
        Some(page)
    }

    fn table(&mut self, page: &str) -> &PageTable {
        if !self.pages.contains_key(page) {
            let table = self.load_table(page);
            self.pages.insert(page.to_string(), table);
        }
        &self.pages[page]
    }

    fn load_table(&mut self, name: &str) -> PageTable {
        if name.is_empty() || name.contains(char::is_whitespace) {
            return PageTable::new();
        }

        if self.summaries().builtins.contains(name) {
            let script = format!("help {name}");
            return run(Command::new("bash").args(["-c", &script]))
                .map(|text| parse_page(&text))
                .unwrap_or_default();
        }

        let Some(page) = man_page(name) else {
            return PageTable::new();
        };
        let cache = self
            .cache_dir
            .as_ref()
            .map(|dir| dir.join(format!("{name}.gz")));
        if let Some(table) = cache.as_deref().and_then(|file| read_cache(file, &page)) {
            return table;
        }
        if page.size > MAX_PAGE_BYTES {
            return PageTable::new();
        }
        let table = run(Command::new("man")
            .env("MANWIDTH", "1000")
            .args(["--no-hyphenation", "--no-justification", "-P", "cat"])
            .arg(&page.path))
        .map(|text| parse_page(&text))
        .unwrap_or_default();
        if let Some(file) = cache {
            write_cache(&file, &page, &table);
        }
        table
    }
}

impl Describer for ManDescriber {
    fn command(&mut self, name: &str) -> Option<String> {
        self.summaries().by_name.get(name).cloned()
    }

    fn subcommand(&mut self, command: &[String], name: &str) -> Option<String> {
        if name.is_empty() || name.starts_with('-') || name.contains('/') {
            return None;
        }
        let page = self.page_for(command)?;
        // Its own page, e.g `git-add(1)`
        let own = format!("{page}-{name}");
        if let Some(summary) = self.summaries().by_name.get(&own) {
            return Some(summary.clone());
        }
        self.table(&page).get(name).cloned()
    }

    fn option(&mut self, command: &[String], option: &str) -> Option<String> {
        // `--color=auto` is documented as `--color`.
        let key = option.split(['=', '[']).next().unwrap_or(option);
        if !key.starts_with('-') || key.len() < 2 {
            return None;
        }
        let page = self.page_for(command)?;
        self.table(&page).get(key).cloned()
    }
}

/// The typed words that may form a page name, so `page_for` e.g.
/// `git-remote-add` for `git remote add`.
fn page_names(command: &[String]) -> Vec<&str> {
    let mut words = command.iter().map(String::as_str);
    let Some(first) = words.next() else {
        return Vec::new();
    };
    let Some(name) = Path::new(first).file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    std::iter::once(name)
        .chain(words.filter(|word| !word.starts_with('-')))
        .collect()
}

/// Everything a page documents: options and the subcommands listed.
fn parse_page(text: &str) -> PageTable {
    let mut table = parse_options(text);
    table.extend(parse_subcommands(text));
    table
}

/// When the manual sytem and user indexes were last rebuilt.
type IndexStamp = [Option<SystemTime>; 2];

fn index_stamp() -> IndexStamp {
    let user = crate::config::data_dir().map(|dir| dir.join("man/index.db"));
    let modified = |path: &Path| std::fs::metadata(path).ok()?.modified().ok();
    [
        modified(Path::new(MAN_INDEX)),
        user.as_deref().and_then(modified),
    ]
}

/// A man page on disk.
struct Page {
    path: PathBuf,
    mtime: u64,
    size: u64,
}

/// The page `man` would show for `name`.
/// e.g "man -w -- cat" -> "/usr/share/man/man1/rust-cat.1.gz"
fn man_page(name: &str) -> Option<Page> {
    let path = PathBuf::from(run(Command::new("man").args(["-w", "--", name]))?.trim());
    let meta = std::fs::metadata(&path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(Page {
        path,
        mtime,
        size: meta.len(),
    })
}

// The on-disk cache:
// The first line the source manpage and its mtime.
//
// then one line per option/subcommand:
// `entry<TAB>description` per line, gzip compressed.
fn read_cache(file: &Path, page: &Page) -> Option<PageTable> {
    let bytes = std::fs::read(file).ok()?;
    let mut text = String::new();
    GzDecoder::new(bytes.as_slice())
        .read_to_string(&mut text)
        .ok()?;
    let mut lines = text.lines();
    if lines.next()? != cache_key(page) {
        return None;
    }
    Some(
        lines
            .filter_map(|line| line.split_once('\t'))
            .map(|(flag, description)| (flag.to_string(), description.to_string()))
            .collect(),
    )
}

fn write_cache(file: &Path, page: &Page, table: &PageTable) {
    let mut text = cache_key(page);
    text.push('\n');
    for (flag, description) in table {
        text.push_str(flag);
        text.push('\t');
        text.push_str(description);
        text.push('\n');
    }

    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    if encoder.write_all(text.as_bytes()).is_ok()
        && let Ok(bytes) = encoder.finish()
    {
        let _ = std::fs::write(file, bytes);
    }
}

// Our cache format version.
const CACHE_FORMAT: u32 = 3;

fn cache_key(page: &Page) -> String {
    format!("{CACHE_FORMAT}\t{}\t{}", page.path.display(), page.mtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_option_is_looked_up_without_its_value() {
        let mut describer = ManDescriber::new(None);
        describer.pages.insert(
            "widget".into(),
            PageTable::from([("--color".to_string(), "colour it".to_string())]),
        );
        let widget = ["widget".to_string()];
        assert_eq!(
            describer.option(&widget, "--color=auto").as_deref(),
            Some("colour it")
        );
        assert_eq!(
            describer
                .option(&["/usr/bin/widget".to_string()], "--color")
                .as_deref(),
            Some("colour it"),
            "a path is documented under its last part"
        );
        assert_eq!(describer.option(&widget, "plain"), None);
    }

    /// A describer that knows these pages exist, without asking `whatis`.
    fn knowing(pages: &[(&str, &str)]) -> ManDescriber {
        let mut describer = ManDescriber::new(None);
        describer.summaries = Some(Summaries {
            by_name: pages
                .iter()
                .map(|(name, summary)| (name.to_string(), summary.to_string()))
                .collect(),
            builtins: HashSet::new(),
            // Current, so nothing is re-read from the real index.
            index_stamp: index_stamp(),
        });
        describer
    }

    #[test]
    fn the_cache_directory_is_emptied_and_measured() {
        let dir = std::env::temp_dir().join(format!("reedline-bash-clear-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch dir");
        std::fs::write(dir.join("git"), "12345").expect("a file");
        std::fs::write(dir.join("ls"), "123").expect("a file");
        assert_eq!(clear_cache_dir(&dir).expect("clearing"), (2, 8));
        assert_eq!(clear_cache_dir(&dir).expect("clearing again"), (0, 0));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            clear_cache_dir(&dir).expect("a missing dir is an empty one"),
            (0, 0)
        );
    }

    #[test]
    fn a_subcommand_is_described_by_its_own_page() {
        let mut describer = knowing(&[
            ("git", "the stupid content tracker"),
            ("git-add", "Add file contents to the index"),
        ]);
        let git = ["git".to_string()];
        assert_eq!(
            describer.subcommand(&git, "add").as_deref(),
            Some("Add file contents to the index")
        );
        // A page already looked at, with no listing: nothing to fall back on.
        describer.pages.insert("git".into(), PageTable::new());
        assert_eq!(describer.subcommand(&git, "status"), None, "no such page");
        describer.pages.insert(
            "git".into(),
            PageTable::from([(
                "status".to_string(),
                "Show the working tree status".to_string(),
            )]),
        );
        assert_eq!(
            describer.subcommand(&git, "status").as_deref(),
            Some("Show the working tree status"),
            "listed in the parent's page instead"
        );
        assert_eq!(describer.subcommand(&[], "add"), None);
        assert_eq!(
            describer
                .subcommand(
                    &["git".to_string(), "-C".to_string(), "dir".to_string()],
                    "add"
                )
                .as_deref(),
            Some("Add file contents to the index"),
            "an option and its argument before it are skipped"
        );
    }

    #[test]
    fn the_options_of_a_subcommand_come_from_its_page() {
        let mut describer = knowing(&[("git", ""), ("git-add", "")]);
        describer.pages.insert(
            "git-add".into(),
            PageTable::from([("--verbose".to_string(), "be verbose".to_string())]),
        );
        describer.pages.insert(
            "git".into(),
            PageTable::from([("--version".to_string(), "print it".to_string())]),
        );
        let words =
            |line: &str| -> Vec<String> { line.split_whitespace().map(str::to_string).collect() };
        assert_eq!(
            describer.option(&words("git add"), "--verbose").as_deref(),
            Some("be verbose")
        );
        assert_eq!(
            describer
                .option(&words("git -C dir add"), "--verbose")
                .as_deref(),
            Some("be verbose"),
            "an option before the subcommand is skipped"
        );
        assert_eq!(
            describer
                .option(&words("git status"), "--version")
                .as_deref(),
            Some("print it"),
            "no page for the subcommand, so the command's own"
        );
    }

    #[test]
    fn the_disk_cache_is_keyed_by_the_page_it_came_from() {
        let dir = std::env::temp_dir().join(format!("reedline-bash-test-{}", std::process::id()));
        let file = dir.join("widget");
        let page = Page {
            path: PathBuf::from("/usr/share/man/man1/widget.1.gz"),
            mtime: 1000,
            size: 1,
        };
        let table = PageTable::from([("-x".to_string(), "does x".to_string())]);

        write_cache(&file, &page, &table);
        assert_eq!(read_cache(&file, &page), Some(table));

        let newer = Page {
            mtime: 2000,
            ..page
        };
        assert_eq!(
            read_cache(&file, &newer),
            None,
            "a rewritten page is re-read"
        );

        // A table from an older build, keyed without the format version.
        let old = format!("{}\t{}\n-x\tstale\n", newer.path.display(), newer.mtime);
        std::fs::write(&file, old).expect("writing the old table");
        assert_eq!(
            read_cache(&file, &newer),
            None,
            "an older format is re-read"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
