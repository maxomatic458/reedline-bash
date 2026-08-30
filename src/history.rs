//! Read-only view of the bash history

use reedline::{
    CommandLineSearch, History, HistoryItem, HistoryItemId, HistorySessionId, ReedlineError,
    ReedlineErrorVariants, Result, SearchDirection, SearchFilter, SearchQuery,
};

/// Source of the history entries.
#[allow(clippy::len_without_is_empty, reason = "a bound to index against")]
pub trait HistorySource: Send {
    /// How many commands the shell is holding.
    fn len(&self) -> usize;

    /// The command at `index`, counting from the oldest.
    fn get(&self, index: usize) -> Option<&[u8]>;
}

/// Bash itself.
pub struct BashSource;

impl HistorySource for BashSource {
    fn len(&self) -> usize {
        // SAFETY: single-threaded, and the shell is blocked waiting on us.
        unsafe { crate::bash::symbols::history_len() }
    }

    fn get(&self, index: usize) -> Option<&[u8]> {
        // SAFETY: as above.
        unsafe { crate::bash::symbols::history_line(index) }
    }
}

/// The shell's history.
pub struct BashHistory<S: HistorySource> {
    source: S,
    /// How many of the newest entries can be reached.
    window: usize,
}

impl<S: HistorySource> BashHistory<S> {
    pub fn new(source: S, window: usize) -> Self {
        BashHistory { source, window }
    }

    /// Which entries answer `query`.
    fn matching(&self, query: &SearchQuery) -> Result<Vec<usize>> {
        unsupported(query)?;

        let len = self.source.len() as i64;
        // Only the newest `window` entries are reachable.
        let first = len - i64::try_from(self.window).unwrap_or(i64::MAX).min(len);

        // Exclusive bounds. An inverted range is empty on its own.
        let (from, to) = match query.direction {
            SearchDirection::Backward => (query.end_id, query.start_id),
            SearchDirection::Forward => (query.start_id, query.end_id),
        };

        let from = from.map_or(0, |id| id.0.saturating_add(1)).max(first);
        let to = to.map_or(len - 1, |id| id.0.saturating_sub(1)).min(len - 1);

        let span = (from..=to).map(|index| index as usize);
        let limit = query.limit.map_or(usize::MAX, |limit| limit as usize);
        let keep = |&index: &usize| {
            self.source
                .get(index)
                .is_some_and(|line| matches(&query.filter, line))
        };

        Ok(match query.direction {
            SearchDirection::Backward => span.rev().filter(keep).take(limit).collect(),
            SearchDirection::Forward => span.filter(keep).take(limit).collect(),
        })
    }

    fn item(&self, index: usize) -> Option<HistoryItem> {
        let line = self.source.get(index)?;
        Some(HistoryItem {
            id: Some(HistoryItemId::new(index as i64)),
            command_line: String::from_utf8_lossy(line).into_owned(),
            start_timestamp: None,
            session_id: None,
            hostname: None,
            cwd: None,
            duration: None,
            exit_status: None,
            more_info: None,
        })
    }
}

/// Whether `line` is what the query asked for.
fn matches(filter: &SearchFilter, line: &[u8]) -> bool {
    // As bytes: a line the shell holds need not be valid UTF-8.
    match &filter.command_line {
        Some(CommandLineSearch::Prefix(text)) => line.starts_with(text.as_bytes()),
        Some(CommandLineSearch::Substring(text)) => contains(line, text.as_bytes()),
        Some(CommandLineSearch::Exact(text)) => line == text.as_bytes(),
        None => true,
    }
}

fn contains(line: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || line.windows(needle.len()).any(|window| window == needle)
}

fn unsupported(query: &SearchQuery) -> Result<()> {
    if query.start_time.is_some() || query.end_time.is_some() {
        return Err(no("filtering by time"));
    }
    if query.filter.hostname.is_some()
        || query.filter.cwd_exact.is_some()
        || query.filter.cwd_prefix.is_some()
        || query.filter.exit_successful.is_some()
    {
        return Err(no("filtering by extra info"));
    }
    Ok(())
}

fn no(feature: &'static str) -> ReedlineError {
    ReedlineError(ReedlineErrorVariants::HistoryFeatureUnsupported {
        history: "BashHistory",
        feature,
    })
}

impl<S: HistorySource> History for BashHistory<S> {
    /// Dropped: the shell records the line itself.
    fn save(&mut self, h: HistoryItem) -> Result<HistoryItem> {
        Ok(h)
    }

    fn load(&self, id: HistoryItemId) -> Result<HistoryItem> {
        usize::try_from(id.0)
            .ok()
            .filter(|&index| index < self.source.len())
            .and_then(|index| self.item(index))
            .ok_or(ReedlineError(ReedlineErrorVariants::OtherHistoryError(
                "Item does not exist",
            )))
    }

    fn count(&self, query: SearchQuery) -> Result<i64> {
        Ok(self.matching(&query)?.len() as i64)
    }

    fn search(&self, query: SearchQuery) -> Result<Vec<HistoryItem>> {
        Ok(self
            .matching(&query)?
            .into_iter()
            .filter_map(|index| self.item(index))
            .collect())
    }

    fn update(
        &mut self,
        _id: HistoryItemId,
        _updater: &dyn Fn(HistoryItem) -> HistoryItem,
    ) -> Result<()> {
        Err(no("updating entries"))
    }

    /// `history -c` reaches us by emptying the shell's list.
    fn clear(&mut self) -> Result<()> {
        Err(no("clearing entries"))
    }

    fn delete(&mut self, _h: HistoryItemId) -> Result<()> {
        Err(no("deleting entries"))
    }

    /// Nothing to write: the shell owns `HISTFILE`.
    fn sync(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn session(&self) -> Option<HistorySessionId> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeSource(Vec<String>);

    impl FakeSource {
        fn with(lines: &[&str]) -> Self {
            FakeSource(lines.iter().map(|line| line.to_string()).collect())
        }
    }

    impl HistorySource for FakeSource {
        fn len(&self) -> usize {
            self.0.len()
        }

        fn get(&self, index: usize) -> Option<&[u8]> {
            self.0.get(index).map(String::as_bytes)
        }
    }

    fn history(lines: &[&str]) -> BashHistory<FakeSource> {
        BashHistory::new(FakeSource::with(lines), 100)
    }

    fn commands<S: HistorySource>(history: &BashHistory<S>, query: SearchQuery) -> Vec<String> {
        history
            .search(query)
            .expect("the query is one this history answers")
            .into_iter()
            .map(|item| item.command_line)
            .collect()
    }

    fn everything<S: HistorySource>(history: &BashHistory<S>) -> Vec<String> {
        commands(
            history,
            SearchQuery::everything(SearchDirection::Forward, None),
        )
    }

    #[test]
    fn the_shells_list_is_what_reedline_searches() {
        let history = history(&["echo one", "echo two"]);
        assert_eq!(everything(&history), ["echo one", "echo two"]);
    }

    #[test]
    fn an_emptied_list_has_nothing_to_offer() {
        let history = history(&[]);
        assert!(everything(&history).is_empty());
        assert_eq!(history.count_all().expect("counting nothing"), 0);
    }

    #[test]
    fn the_newest_entry_comes_back_first_when_reading_backwards() {
        let history = history(&["echo one", "echo two"]);
        let query = SearchQuery::last_with_search(SearchFilter::anything(None));
        assert_eq!(commands(&history, query), ["echo two"]);
    }

    #[test]
    fn a_prefix_search_finds_the_most_recent_match() {
        let history = history(&["echo old", "true", "echo new"]);
        let query = SearchQuery::last_with_prefix("echo ".into(), None);
        assert_eq!(commands(&history, query), ["echo new"]);
    }

    #[test]
    fn a_prefix_search_does_not_match_mid_line() {
        let history = history(&["echo new", "run echo now"]);
        let query = SearchQuery::last_with_prefix("echo".into(), None);
        assert_eq!(commands(&history, query), ["echo new"]);
    }

    #[test]
    fn an_exact_search_wants_the_whole_line() {
        let history = history(&["echo", "echo one"]);
        let filter = SearchFilter::from_text_search(CommandLineSearch::Exact("echo".into()), None);
        assert_eq!(
            commands(&history, SearchQuery::last_with_search(filter)),
            ["echo"]
        );
    }

    #[test]
    fn a_substring_search_matches_anywhere_in_the_line() {
        let history = history(&["echo one", "run echo now"]);
        let filter =
            SearchFilter::from_text_search(CommandLineSearch::Substring("echo".into()), None);
        assert_eq!(
            commands(&history, SearchQuery::last_with_search(filter)),
            ["run echo now"]
        );
    }

    #[test]
    fn a_substring_search_for_nothing_matches_every_line() {
        // Ctrl-R, before anything is typed into it.
        let history = history(&["echo one", "echo two"]);
        let filter =
            SearchFilter::from_text_search(CommandLineSearch::Substring(String::new()), None);
        assert_eq!(
            commands(
                &history,
                SearchQuery::everything(SearchDirection::Forward, None)
            )
            .len(),
            2
        );
        assert_eq!(
            commands(&history, SearchQuery::last_with_search(filter)),
            ["echo two"]
        );
    }

    #[test]
    fn a_query_it_cannot_answer_is_refused_rather_than_guessed() {
        let history = history(&["echo one"]);
        let mut by_place = SearchQuery::everything(SearchDirection::Forward, None);
        by_place.filter.cwd_prefix = Some("/home".to_string());
        assert!(history.search(by_place).is_err());
    }

    #[test]
    fn an_id_is_where_the_entry_sits_in_the_shells_list() {
        let history = history(&["echo one", "echo two"]);
        let found = history
            .search(SearchQuery::everything(SearchDirection::Forward, None))
            .expect("a plain search");
        assert_eq!(found[1].id, Some(HistoryItemId::new(1)));
        assert_eq!(
            history
                .load(HistoryItemId::new(1))
                .expect("an id search just handed back")
                .command_line,
            "echo two"
        );
    }

    #[test]
    fn walking_back_from_an_id_skips_what_was_already_seen() {
        let history = history(&["one", "two", "three"]);
        let mut query = SearchQuery::last_with_search(SearchFilter::anything(None));
        query.start_id = Some(HistoryItemId::new(2));
        assert_eq!(commands(&history, query), ["two"]);
    }

    #[test]
    fn walking_forward_from_an_id_moves_off_it() {
        let history = history(&["one", "two", "three"]);
        let mut query = SearchQuery::everything(SearchDirection::Forward, None);
        query.start_id = Some(HistoryItemId::new(0));
        query.limit = Some(1);
        assert_eq!(commands(&history, query), ["two"]);
    }

    #[test]
    fn an_id_from_outside_the_window_does_not_reach_back_past_it() {
        let history = BashHistory::new(FakeSource::with(&["one", "two", "three"]), 2);
        let mut query = SearchQuery::everything(SearchDirection::Forward, None);
        query.start_id = Some(HistoryItemId::new(-1));
        assert_eq!(commands(&history, query), ["two", "three"]);
    }

    #[test]
    fn only_the_newest_window_entries_are_reachable() {
        let history = BashHistory::new(FakeSource::with(&["one", "two", "three"]), 2);
        assert_eq!(everything(&history), ["two", "three"]);
    }

    #[test]
    fn a_window_of_zero_reaches_nothing() {
        let history = BashHistory::new(FakeSource::with(&["one"]), 0);
        assert!(everything(&history).is_empty());
    }

    #[test]
    fn a_line_the_shell_holds_need_not_be_valid_utf8() {
        struct Raw;
        impl HistorySource for Raw {
            fn len(&self) -> usize {
                1
            }
            fn get(&self, _index: usize) -> Option<&[u8]> {
                Some(b"echo \xff")
            }
        }
        let history = BashHistory::new(Raw, 100);
        let query = SearchQuery::last_with_prefix("echo ".into(), None);
        assert_eq!(commands(&history, query), ["echo \u{fffd}"]);
    }

    #[test]
    fn an_id_past_the_end_of_the_list_is_not_read() {
        // Answers out of range, as reading past the real list would.
        struct Unchecked;
        impl HistorySource for Unchecked {
            fn len(&self) -> usize {
                1
            }
            fn get(&self, _index: usize) -> Option<&[u8]> {
                Some(b"whatever")
            }
        }
        let history = BashHistory::new(Unchecked, 100);
        assert!(history.load(HistoryItemId::new(1)).is_err());
        assert!(history.load(HistoryItemId::new(-1)).is_err());
        assert_eq!(everything(&history), ["whatever"]);

        let mut past_the_end = SearchQuery::everything(SearchDirection::Forward, None);
        past_the_end.end_id = Some(HistoryItemId::new(99));
        assert_eq!(commands(&history, past_the_end), ["whatever"]);
    }

    #[test]
    fn a_range_that_ends_before_it_starts_holds_nothing() {
        let history = history(&["one", "two", "three"]);
        let mut query = SearchQuery::everything(SearchDirection::Forward, None);
        query.start_id = Some(HistoryItemId::new(2));
        query.end_id = Some(HistoryItemId::new(1));
        assert!(commands(&history, query).is_empty());
    }
}
