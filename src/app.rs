use crate::{
    cache::DesktopEntryCache,
    config::Config,
    content::{ClassificationError, Content, ContentClassifier},
    history::History,
    input::{self, InputContext},
    search::{self, search_path_exact_match, sort_search_results, SearchMatch, SearchMatchKind},
    smart_content::{Action, ReadyContent},
    ui::Ui,
    units::{convert, default_unit_mapping, Unit},
    util::{copy, launch_orphan},
    x::Display,
};
use std::{
    borrow::Borrow,
    collections::HashMap,
    ops::Deref,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};
use x11::xlib::{ButtonPress, KeyPress, LASTEvent, XEvent, XFilterEvent};

const SIGNAL_EVENT: i32 = LASTEvent + 1;

pub enum Signal {
    SearchTextChanged(String),
    CursorPositionChanged((i32, i32)),
    SwapFocus,
    Quit,
    Commit(Option<usize>),
    DeleteEntry(usize),
}

pub fn send_signal(display: &Display, sender: &Sender<Signal>, signal: Signal) {
    if let Err(error) = sender.send(signal) {
        eprintln!("Signal send error: {error}");
    }
    let event = unsafe {
        let mut event: XEvent = std::mem::zeroed();
        event.any.type_ = SIGNAL_EVENT;
        event
    };
    display.push_event(event);
}

pub struct App {
    display: Display,
    signal_receiver: Receiver<Signal>,
    ui: Ui,
    ic: InputContext,
    cache: Arc<Mutex<DesktopEntryCache>>,
    search_results: Vec<SearchMatch>,
    history: History,
    search_text: String,
    content_classifier: ContentClassifier,
    default_unit_mapping: HashMap<Unit, Unit>,
}

impl App {
    pub fn new(display: Display, cache: Arc<Mutex<DesktopEntryCache>>, config: Config) -> Self {
        let history = History::load(cache.lock().unwrap().borrow(), config.history_entries);
        let (signal_sender, signal_receiver) = channel();
        let ui = Ui::new(&display, signal_sender, cache.clone(), &config);
        let ic = input::init(&display, &ui.main_window);
        Self {
            display,
            signal_receiver,
            ui,
            ic,
            cache,
            search_results: Vec::new(),
            history,
            search_text: String::new(),
            content_classifier: ContentClassifier::new(config.smart_content_options),
            default_unit_mapping: default_unit_mapping(&config.default_currency).mapping,
        }
    }

    fn process_smart_content(
        &self,
        classified: Result<Option<Content>, ClassificationError>,
        s: &str,
    ) -> Option<ReadyContent> {
        match classified {
            Ok(Some(Content::BasicExpression(value))) => ReadyContent::Expression(value),
            Ok(Some(Content::LeadExpression(maybe_value))) => match maybe_value {
                Ok(value) => ReadyContent::Expression(value),
                Err(error) => ReadyContent::Error(format!("{}", error)),
            },
            Ok(Some(Content::DefaultConversion(value, from))) => {
                if let Some(to) = self.default_unit_mapping.get(&from) {
                    let value = convert(value, from.into(), to.clone().into());
                    ReadyContent::Conversion(value, from.into(), to.clone().into())
                } else {
                    ReadyContent::Error(format!("No default conversion for {from}"))
                }
            }
            Ok(Some(Content::Conversion(value, maybe_from, to))) => {
                if let Some(from) =
                    maybe_from.or_else(|| self.default_unit_mapping.get(&to).copied())
                {
                    let value = convert(value, from.into(), to.into());
                    ReadyContent::Conversion(value, from.into(), to.into())
                } else {
                    ReadyContent::Error(format!("No default conversion for {to}"))
                }
            }
            Ok(Some(Content::Path)) => {
                let path = &s[1..].trim();
                ReadyContent::Action(Action::Path, "Open", path.to_string())
            }
            Ok(Some(Content::URL)) => ReadyContent::Action(Action::Web, "Open", s.to_string()),
            Ok(Some(Content::Command)) => {
                let command = &s[1..].trim();
                ReadyContent::Action(Action::Run, "Run", command.to_string())
            }
            Ok(None) => return None,
            Err(error) => ReadyContent::Error(format!("{}", error)),
        }
        .into()
    }

    pub fn run(&mut self) {
        if !self.history.is_empty() {
            self.ui.set_items(self.history.entries(), "");
        }
        self.ui.redraw();
        self.display.sync(true);
        let mut running = true;
        let mut event: XEvent = unsafe { std::mem::zeroed() };
        while running {
            self.display.next_event(&mut event);
            if unsafe { event.type_ } == SIGNAL_EVENT {
                // Need to catch these before XFilterEvent
                let maybe_signal = self.signal_receiver.recv();
                if let Err(error) = maybe_signal {
                    println!("Signal receive error: {error}");
                    continue;
                }
                match maybe_signal.unwrap() {
                    Signal::SearchTextChanged(text) => {
                        if text == self.search_text {
                            continue;
                        }
                        ///////////////////////////////////////////////////////
                        // Smart Content
                        self.ui.set_smart_content(
                            self.process_smart_content(
                                self.content_classifier.classify(&text),
                                &text,
                            ),
                        );
                        let text = if text.starts_with('$') {
                            text[1..].trim().to_string()
                        } else {
                            text
                        };
                        ///////////////////////////////////////////////////////
                        // Search
                        if text.is_empty() {
                            self.search_text.clear();
                            self.search_results.clear();
                            if self.history.is_empty() {
                                self.ui.set_items::<SearchMatch>(&[], "");
                            } else {
                                self.ui.set_items(self.history.entries(), "");
                            }
                            continue;
                        }
                        // Only searching for a subset with a short search text will likely
                        // results in not finding things we want to find with the current text.
                        if self.search_text.len() >= 3 && text.starts_with(&self.search_text) {
                            self.search_results = search::search(
                                &text,
                                self.cache.clone(),
                                Some(std::mem::take(&mut self.search_results)),
                            );
                        } else {
                            self.search_results = search::search(&text, self.cache.clone(), None);
                        }
                        sort_search_results(
                            &mut self.search_results,
                            self.history.borrow().desktop_ids(),
                        );
                        self.ui.set_items(&self.search_results, &text);
                        self.search_text = text;
                    }
                    Signal::CursorPositionChanged((x, y)) => {
                        self.ic.set_cursor_position(x, y);
                    }
                    Signal::SwapFocus => {
                        self.ui.swap_focus();
                    }
                    Signal::Quit => {
                        running = false;
                    }
                    Signal::Commit(id) => {
                        // If there is smart content, pressing enter with the
                        // entry focused should interact with it.
                        if let Some(id) = id.or_else(|| {
                            if self.ui.showing_useful_smart_content() {
                                None
                            } else {
                                Some(0)
                            }
                        }) {
                            if let Some(exec) = self.get_exec(id) {
                                self.launch(exec);
                                if self.search_results.is_empty() {
                                    self.history.renew(id);
                                } else {
                                    self.history.add(
                                        self.search_results[id].unwrap(),
                                        self.cache.lock().unwrap().borrow(),
                                    );
                                }
                            }
                            running = false;
                        } else if let Some(action) = self.ui.smart_content.commit() {
                            use crate::smart_content::SmartContentCommitAction::*;
                            match action {
                                Copy(text) => {
                                    copy(&text);
                                }
                                OpenPath(path) => launch_orphan(&format!("xdg-open {path}")),
                                OpenWeb(url) => 'out: {
                                    // We are a lot looser with URLs than
                                    // xdg-open (at least in loose URL mod), so
                                    // we really want to open it manually.
                                    if let Ok(browser) = std::env::var("BROWSER") {
                                        launch_orphan(&format!("{browser} {url}"))
                                    } else if url.starts_with("http") {
                                        launch_orphan(&format!("xdg-open {url}"))
                                    } else {
                                        println!(
                                            "$BROWSER not set nad URL doesn't look xdg-openable; trying some common browsers"
                                        );
                                        for browser in
                                            ["firefox", "chromium", "google-chrome", "epiphany"]
                                        {
                                            println!("  {browser}");
                                            if search_path_exact_match(browser) {
                                                println!("   -> Found");
                                                launch_orphan(&format!("{browser} {url}"));
                                                break 'out;
                                            }
                                        }
                                        println!("None found, trying xdg-open");
                                        launch_orphan(&format!("xdg-open {url}"));
                                    }
                                }
                                Run(command) => launch_orphan(&command),
                            }
                            running = false;
                        }
                    }
                    Signal::DeleteEntry(id) => {
                        if self.search_results.is_empty() && self.search_text.is_empty() {
                            self.history.delete(id, self.cache.lock().unwrap().borrow());
                        }
                        self.ui.set_items(self.history.entries(), "");
                    }
                }
                continue;
            }
            if unsafe { XFilterEvent(&mut event, 0) != 0 } {
                continue;
            }
            #[allow(non_upper_case_globals)]
            match unsafe { event.type_ } {
                KeyPress => {
                    let mut event = unsafe { event.key };
                    if let Some(key) = input::translate_key(&event) {
                        self.ui.key_press(key);
                    } else if let Some(str) = self.ic.lookup(&mut event) {
                        self.ui.text_input(str);
                    }
                }
                ButtonPress => {
                    self.ui.button_press(unsafe { &mut event.button });
                }
                _ => continue,
            }
        }
        self.history.store();
    }

    fn get_exec(&mut self, id: usize) -> Option<String> {
        if !self.search_results.is_empty() {
            Some(match &self.search_results[id].unwrap() {
                SearchMatchKind::PathEntry(path) => path.to_str().unwrap().to_string(),
                SearchMatchKind::DeskopEntry(entry) => {
                    self.cache.lock().unwrap().get_entry(entry.id).exec.clone()
                }
            })
        } else if !self.history.is_empty() && self.search_text.is_empty() {
            use crate::history::Entry;
            Some(match &self.history.entries()[id] {
                Entry::Path(path) => path.to_str().unwrap().to_string(),
                Entry::DesktopEntry(file_name) => {
                    let guard = self.cache.lock().unwrap();
                    let cache = guard.deref();
                    let id = cache.find_file(file_name).unwrap();
                    cache.get_entry(id).exec.clone()
                }
            })
        } else {
            None
        }
    }

    fn launch(&self, exec: String) {
        launch_orphan(&exec);
    }
}
