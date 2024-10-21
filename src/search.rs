use crate::{
    cache::{DesktopEntryCache, MatchField},
    list_view::Render,
    res::Svg,
    ui::colors,
};
use std::{
    cell::OnceCell,
    cmp::Ordering,
    collections::HashMap,
    os::unix::prelude::PermissionsExt,
    path::PathBuf,
    sync::{
        mpsc::{channel, Sender},
        Arc, Mutex,
    },
    thread,
};

#[derive(Copy, Clone)]
pub enum MatchKind {
    Exact,
    Similar(f64),
}

mod scores {
    // Desktop entry weights
    pub const LOCALIZED_NAME_WEIGHT: f64 = 1.4;
    pub const NAME_WEIGHT: f64 = 1.2;
    pub const LOCALIZED_GENERIC_NAME_WEIGHT: f64 = 1.3;
    pub const GENERIC_NAME_WEIGHT: f64 = 1.1;
    pub const FILE_NAME_WEIGHT: f64 = 0.8;
    // Path weights
    pub const PATH_WEIGHT: f64 = 1.0;

    // Match kind weights
    pub const EXACT_BASE: f64 = 1.2;
    // MatchKind::Similar uses the similarity (in range [0:1]) as base.

    /// Two scores with a delta less than or equal to this are considered to be equal.
    pub const EQUAL_THREHOLD: f64 = 1e-3;
}

pub const SIMILARITY_THRESHHOLD: f64 = 0.75;

pub struct DesktopEntryData {
    pub id: usize,
    pub name: String,
    pub match_name: Option<String>,
}

pub enum SearchMatchKind {
    DeskopEntry(DesktopEntryData),
    PathEntry(PathBuf),
}

pub struct SearchMatch {
    match_: SearchMatchKind,
    score: f64,
    is_in_history: bool,
}

impl SearchMatch {
    fn new(match_: SearchMatchKind, score: f64) -> Self {
        Self {
            match_,
            score,
            // This is set in `sort_search_results` when the score is boosted.
            is_in_history: false,
        }
    }

    pub fn unwrap(&self) -> &SearchMatchKind {
        &self.match_
    }

    fn name(&self) -> &str {
        match &self.match_ {
            SearchMatchKind::PathEntry(path) => path.file_name().unwrap().to_str().unwrap(),
            SearchMatchKind::DeskopEntry(entry) => entry.name.as_str(),
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        if (self.score - other.score).abs() <= scores::EQUAL_THREHOLD {
            self.name().cmp(other.name())
        } else {
            other.score.total_cmp(&self.score)
        }
    }
}

impl Render for SearchMatch {
    fn markup(&self, search: &str, _cache: &DesktopEntryCache) -> String {
        match &self.match_ {
            SearchMatchKind::DeskopEntry(entry) => {
                if let Some(match_name) = &entry.match_name {
                    format!(
                        "{} <span color=\"{}\">({})</span>",
                        entry.name,
                        colors::LIST_MATCH_NAME,
                        highlight_match(match_name, search)
                    )
                } else {
                    highlight_match(&entry.name, search)
                }
            }
            SearchMatchKind::PathEntry(path) => {
                highlight_match(path.file_name().unwrap().to_str().unwrap(), search)
            }
        }
    }

    fn icon(&self, cache: &DesktopEntryCache) -> Option<Svg> {
        match &self.match_ {
            SearchMatchKind::PathEntry(_) => None,
            SearchMatchKind::DeskopEntry(entry) => cache
                .get_entry(entry.id)
                .icon
                .as_ref()
                .map(|icon_path| Svg::open(icon_path)),
        }
    }

    fn is_in_history(&self) -> bool {
        self.is_in_history
    }
}

fn send_finish(writer: Sender<Option<SearchMatch>>) {
    let mut tries = 0;
    while tries < 10 {
        if writer.send(None).is_ok() {
            return;
        }
        tries += 1;
    }
    panic!("Failed to send finish token {} times.", tries);
}

fn path_entry_score(item: &str, target: &str) -> Option<f64> {
    if item == target {
        Some(scores::EXACT_BASE)
    } else {
        let sim = strsim::jaro_winkler(item, target);
        if sim < SIMILARITY_THRESHHOLD {
            None
        } else {
            Some(sim)
        }
    }
}

fn search_path(name: String, sender: Sender<Option<SearchMatch>>) {
    let paths = std::env::var("PATH").unwrap();
    for path in paths.split(':') {
        if let Ok(dir) = std::fs::read_dir(path) {
            for entry in dir.flatten() {
                if entry.file_type().unwrap().is_file()
                    && entry.metadata().unwrap().permissions().mode() & 0o111 != 0
                {
                    let entry_name = entry.file_name().to_str().unwrap().to_lowercase();
                    if let Some(score) = path_entry_score(&entry_name, &name) {
                        if score >= SIMILARITY_THRESHHOLD {
                            sender
                                .send(Some(SearchMatch::new(
                                    SearchMatchKind::PathEntry(entry.path()),
                                    score * scores::PATH_WEIGHT,
                                )))
                                .ok();
                        }
                    }
                }
            }
        }
    }
    send_finish(sender);
}

fn get_field_scale(field: MatchField) -> f64 {
    match field {
        MatchField::LocalizedName(_) => scores::LOCALIZED_NAME_WEIGHT,
        MatchField::Name(_) => scores::NAME_WEIGHT,
        MatchField::LocalizedGenericName(_) => scores::LOCALIZED_GENERIC_NAME_WEIGHT,
        MatchField::GenericName(_) => scores::GENERIC_NAME_WEIGHT,
        MatchField::FileName(_) => scores::FILE_NAME_WEIGHT,
    }
}

fn desktop_entry_score(field: MatchField) -> f64 {
    match field.into_inner() {
        MatchKind::Exact => scores::EXACT_BASE,
        MatchKind::Similar(sim) => sim * get_field_scale(field),
    }
}

fn search_desktop_entries(
    name: String,
    sender: Sender<Option<SearchMatch>>,
    cache: Arc<Mutex<DesktopEntryCache>>,
    previous: Option<Vec<SearchMatch>>,
) {
    let cache = cache.as_ref().lock().unwrap();
    let matches = if let Some(previous) = previous {
        cache.find_subset(
            &name,
            previous
                .into_iter()
                .filter(|m| matches!(m.match_, SearchMatchKind::DeskopEntry(_)))
                .map(|m| match m.match_ {
                    SearchMatchKind::DeskopEntry(entry) => entry.id,
                    _ => unreachable!(),
                }),
        )
    } else {
        cache.find_all(&name)
    };
    for match_ in matches {
        let entry = cache.get_entry(match_.id);
        let score = desktop_entry_score(match_.field);
        let name = entry.name.clone();
        let matched_field = entry.get_field(match_.field);
        let match_name = if name == matched_field {
            None
        } else {
            Some(matched_field.to_owned())
        };
        sender
            .send(Some(SearchMatch::new(
                SearchMatchKind::DeskopEntry(DesktopEntryData {
                    id: match_.id,
                    name: entry.name.clone(),
                    match_name,
                }),
                score,
            )))
            .ok();
    }
    send_finish(sender);
}

pub fn search(
    name: &str,
    cache: Arc<Mutex<DesktopEntryCache>>,
    previous: Option<Vec<SearchMatch>>,
) -> Vec<SearchMatch> {
    let (sender, receiver) = channel();
    let mut results: Vec<SearchMatch> = Vec::new();
    // Number of running search functions
    let mut running = 0;
    macro_rules! begin {
    ($function:ident $(, $opt:expr)*) => {{
      let my_name = name.to_lowercase ();
      let my_writer = sender.clone ();
      thread::spawn (|| $function (my_name, my_writer, $($opt),*));
      running += 1;
    }}
  }
    begin!(search_path);
    begin!(search_desktop_entries, cache, previous);
    while running != 0 {
        match receiver.recv() {
            Ok(result_or_finish_token) => {
                if let Some(result) = result_or_finish_token {
                    results.push(result);
                } else {
                    running -= 1;
                }
            }
            Err(error) => {
                eprintln!("Receive error: {error}");
            }
        }
    }
    results
}

/// Sorts the search results. If any of the results is in the history its score
/// heavily adjusted toward how recent it is in the history.
pub fn sort_search_results(results: &mut [SearchMatch], history: &HashMap<usize, usize>) {
    for result in results.iter_mut() {
        if let SearchMatchKind::DeskopEntry(data) = &result.unwrap() {
            if let Some(recency) = history.get(&data.id) {
                // Original version:
                // this will always place results in the history above those that are
                // are not, ordering the history results by recency.
                // result.score = 10.0 + *recency as f64;

                // Note that only for old elements (recency 1 or 2) would this not
                // have the same effect as the above implementation
                result.score *= 2.0 * *recency as f64;

                result.is_in_history = true;
            }
        }
    }
    results.sort_by(|a, b| a.compare(b));
}

fn highlight_match(match_str: &str, search: &str) -> String {
    const END_HIGHLIGHT: &str = "</span>";
    let cell = OnceCell::new();
    let begin_highlight =
        cell.get_or_init(|| format!("<span color=\"{}\">", colors::LIST_MATCH_HIGHLIGHT));
    // Assume 75% of chars in search resulting in this: `<span color="#RRGGBB">X</span>`
    let mut result =
        String::with_capacity(match_str.len() + 30 * search.chars().count() * 75 / 100);
    let mut match_chars = match_str.chars();
    let mut search_chars = search.chars().filter(|c| *c != ' ');
    let mut s = search_chars.next().unwrap().to_ascii_lowercase();
    let mut is_highlight = false;
    // Highlight all matching in-order
    for c in match_chars.by_ref() {
        if c.to_ascii_lowercase() == s {
            if !is_highlight {
                is_highlight = true;
                result.push_str(begin_highlight);
            }
            if let Some(next_s) = search_chars.next() {
                s = next_s.to_ascii_lowercase();
            } else {
                result.push(c);
                break;
            }
        } else if is_highlight {
            is_highlight = false;
            result.push_str(END_HIGHLIGHT);
        }
        result.push(c);
    }
    if is_highlight {
        result.push_str(END_HIGHLIGHT);
    }
    result.extend(match_chars);
    result
}
