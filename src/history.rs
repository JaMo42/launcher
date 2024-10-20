use crate::{cache::DesktopEntryCache, list_view::Render, res::Svg, search::SearchMatchKind};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
};

const FILE: &str = "history";
pub const DEFAULT_MAX_SIZE: usize = 100;

#[derive(Serialize, Deserialize, Eq, PartialEq)]
pub enum Entry {
    Path(PathBuf),
    DesktopEntry(String),
}

impl Render for Entry {
    fn markup(&self, _search: &str, cache: &DesktopEntryCache) -> String {
        match self {
            Entry::DesktopEntry(file_name) => {
                let id = cache.find_file(file_name).unwrap();
                cache.get_entry(id).name.clone()
            }
            Entry::Path(path) => path.file_name().unwrap().to_str().unwrap().to_string(),
        }
    }

    fn icon(&self, cache: &DesktopEntryCache) -> Option<Svg> {
        match self {
            Entry::Path(_) => None,
            Entry::DesktopEntry(file_name) => {
                let id = cache.find_file(file_name).unwrap();
                cache
                    .get_entry(id)
                    .icon
                    .as_ref()
                    .map(|icon_path| Svg::open(icon_path))
            }
        }
    }

    // `is_in_history` is not implemented since it's pointless to show that the
    // history entries are in the history when we're only showing the history.
}

pub struct History {
    entries: VecDeque<Entry>,
    // maps IDs in the desktop cache to their recency score.
    desktop_ids: HashMap<usize, usize>,
    next_score: usize,
    max_size: usize,
}

impl History {
    fn new(max_size: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            desktop_ids: HashMap::new(),
            next_score: 0,
            max_size,
        }
    }

    fn dirpath() -> String {
        // no reason storing this, this happens once at start and once when quitting
        format!("{}/.cache/launcher", std::env::var("HOME").unwrap())
    }

    pub fn load(cache: &DesktopEntryCache, max_size: usize) -> Self {
        let pathname = format!("{}/{}", Self::dirpath(), FILE);
        println!("Loading history from {}", pathname);
        if let Ok(history_data) = std::fs::read_to_string(pathname) {
            if history_data.is_empty() {
                return Self::new(max_size);
            }
            let entries: VecDeque<Entry> = ron::from_str(&history_data).unwrap();
            let entries: VecDeque<Entry> = entries
                .into_iter()
                .filter(|e| match e {
                    Entry::Path(path) => std::fs::metadata(path).is_ok(),
                    Entry::DesktopEntry(file_name) => cache.find_file(file_name).is_some(),
                })
                .collect();
            let mut desktop_ids = HashMap::new();
            for (idx, entry) in entries.iter().enumerate() {
                if let Entry::DesktopEntry(file_name) = entry {
                    if let Some(id) = cache.find_file(file_name) {
                        desktop_ids.insert(id, entries.len() - idx);
                    }
                }
            }
            let next_score = entries.len();
            Self {
                entries,
                desktop_ids,
                next_score,
                max_size,
            }
        } else {
            Self::new(max_size)
        }
    }

    pub fn store(&self) {
        let dir = Self::dirpath();
        std::fs::create_dir_all(&dir).unwrap();
        let pathname = format!("{}/{}", dir, FILE);
        let data = ron::to_string(&self.entries).unwrap();
        std::fs::write(&pathname, data).unwrap();
        println!("History saved to {}", pathname);
    }

    pub fn add(&mut self, result: &SearchMatchKind, cache: &DesktopEntryCache) {
        let entry = match result {
            SearchMatchKind::PathEntry(path) => Entry::Path(path.clone()),
            SearchMatchKind::DeskopEntry(entry) => {
                self.desktop_ids.insert(entry.id, self.next_score);
                self.next_score += 1;
                let entry = cache.get_entry(entry.id);
                Entry::DesktopEntry(entry.file_name.clone())
            }
        };
        // Remove old item for the same result
        for idx in 0..self.entries.len() {
            if self.entries[idx] == entry {
                self.entries.remove(idx);
                break;
            }
        }
        // Drop oldest if capacity is filled
        if self.entries.len() == self.max_size {
            self.entries.pop_back();
        }
        self.entries.push_front(entry);
    }

    pub fn desktop_ids(&self) -> &HashMap<usize, usize> {
        &self.desktop_ids
    }

    pub fn entries(&mut self) -> &[Entry] {
        self.entries.make_contiguous();
        self.entries.as_slices().0
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn renew(&mut self, id: usize) {
        let entry = self.entries.remove(id).unwrap();
        self.entries.push_front(entry);
    }

    pub fn delete(&mut self, id: usize, cache: &DesktopEntryCache) {
        if let Entry::DesktopEntry(file_name) = self.entries.remove(id).unwrap() {
            let id = cache.find_file(&file_name).unwrap();
            self.desktop_ids.remove(&id).unwrap();
        }
    }
}
