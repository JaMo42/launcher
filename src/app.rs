use crate::{
  cache::DesktopEntryCache,
  config::Config,
  history::History,
  input::{self, InputContext},
  search::{self, sort_search_results, SearchMatch, SearchMatchKind},
  ui::UI,
  util::launch_orphan,
  x::Display,
};
use std::{
  borrow::Borrow,
  ops::Deref,
  sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, Mutex,
  },
};
use x11::xlib::{ButtonPress, KeyPress, LASTEvent, XEvent, XFilterEvent};

const SIGNAL_EVENT: i32 = LASTEvent + 1;

pub enum Signal {
  SearchTextChanged (String),
  CursorPositionChanged ((i32, i32)),
  SwapFocus,
  Quit,
  Commit (usize),
  DeleteEntry (usize),
}

pub fn send_signal (display: &Display, sender: &Sender<Signal>, signal: Signal) {
  if let Err (error) = sender.send (signal) {
    eprintln! ("Signal send error: {error}");
  }
  let event = unsafe {
    let mut event: XEvent = std::mem::zeroed ();
    event.any.type_ = SIGNAL_EVENT;
    event
  };
  display.push_event (event);
}

pub struct App {
  display: Display,
  signal_receiver: Receiver<Signal>,
  ui: UI,
  ic: InputContext,
  cache: Arc<Mutex<DesktopEntryCache>>,
  search_results: Vec<SearchMatch>,
  history: History,
  search_text: String,
}

impl App {
  pub fn new (display: Display, cache: Arc<Mutex<DesktopEntryCache>>, config: Config) -> Self {
    let history = History::load (cache.lock ().unwrap ().borrow ());
    let (signal_sender, signal_receiver) = channel ();
    let ui = UI::new (&display, signal_sender, cache.clone (), &config);
    let ic = input::init (&display, &ui.main_window);
    Self {
      display,
      signal_receiver,
      ui,
      ic,
      cache,
      search_results: Vec::new (),
      history,
      search_text: String::new (),
    }
  }

  pub fn run (&mut self) {
    if !self.history.is_empty () {
      self.ui.list_view.set_items (self.history.entries (), "");
    }
    self.ui.redraw ();
    self.display.sync (true);
    let mut running = true;
    let mut event: XEvent = unsafe { std::mem::zeroed () };
    while running {
      self.display.next_event (&mut event);
      if unsafe { event.type_ } == SIGNAL_EVENT {
        // Need to catch these before XFilterEvent
        let maybe_signal = self.signal_receiver.recv ();
        if let Err (error) = maybe_signal {
          println! ("Signal receive error: {error}");
          continue;
        }
        match maybe_signal.unwrap () {
          Signal::SearchTextChanged (text) => {
            println! ("Text changed: \"{text}\"");
            if text == self.search_text {
              continue;
            }
            if text.is_empty () {
              self.search_text.clear ();
              self.search_results.clear ();
              if self.history.is_empty () {
                self.ui.list_view.set_items::<SearchMatch> (&[], "");
              } else {
                self.ui.list_view.set_items (self.history.entries (), "");
              }
              continue;
            }
            // Only searching for a subset with a shart search text will likely
            // results in not finding things we want to find with the current text.
            if self.search_text.len () >= 3 && text.starts_with (&self.search_text) {
              self.search_results = search::search (
                &text,
                self.cache.clone (),
                Some (std::mem::take (&mut self.search_results)),
              );
            } else {
              self.search_results = search::search (&text, self.cache.clone (), None);
            }
            sort_search_results (
              &mut self.search_results,
              self.history.borrow ().desktop_ids (),
            );
            self.ui.list_view.set_items (&self.search_results, &text);
            self.search_text = text;
          }
          Signal::CursorPositionChanged ((x, y)) => {
            println! ("Cursor position: {x} {y}");
            self.ic.set_cursor_position (x, y);
          }
          Signal::SwapFocus => {
            println! ("Swap focus");
            self.ui.swap_focus ();
          }
          Signal::Quit => {
            println! ("Quit");
            running = false;
          }
          Signal::Commit (id) => {
            println! ("Commit: {}", id);
            if let Some (exec) = self.get_exec (id) {
              self.launch (exec);
              if self.search_results.is_empty () {
                self.history.renew (id);
              } else {
                self.history.add (
                  &self.search_results[id].unwrap (),
                  self.cache.lock ().unwrap ().borrow (),
                );
              }
              running = false;
            }
          }
          Signal::DeleteEntry (id) => {
            if self.search_results.is_empty () && self.search_text.is_empty () {
              self
                .history
                .delete (id, self.cache.lock ().unwrap ().borrow ());
            }
            self.ui.list_view.set_items (self.history.entries (), "");
            self.ui.list_view.draw ();
          }
        }
        continue;
      }
      if unsafe { XFilterEvent (&mut event, 0) != 0 } {
        continue;
      }
      #[allow(non_upper_case_globals)]
      match unsafe { event.type_ } {
        KeyPress => {
          let mut event = unsafe { event.key };
          if let Some (key) = input::translate_key (&event) {
            self.ui.key_press (key);
          } else if let Some (str) = self.ic.lookup (&mut event) {
            self.ui.text_input (str);
          }
        }
        ButtonPress => {
          self.ui.button_press (unsafe { &mut event.button });
        }
        _ => continue,
      }
    }
    self.history.store ();
  }

  fn get_exec (&mut self, id: usize) -> Option<String> {
    if !self.search_results.is_empty () {
      Some (match &self.search_results[id].unwrap () {
        SearchMatchKind::PathEntry (path) => path.to_str ().unwrap ().to_string (),
        SearchMatchKind::DeskopEntry (entry) => self
          .cache
          .lock ()
          .unwrap ()
          .get_entry (entry.id)
          .exec
          .clone (),
      })
    } else if !self.history.is_empty () && self.search_text.is_empty () {
      use crate::history::Entry;
      Some (match &self.history.entries ()[id] {
        Entry::Path (path) => path.to_str ().unwrap ().to_string (),
        Entry::DesktopEntry (file_name) => {
          let guard = self.cache.lock ().unwrap ();
          let cache = guard.deref ();
          let id = cache.find_file (file_name).unwrap ();
          cache.get_entry (id).exec.clone ()
        }
      })
    } else {
      None
    }
  }

  fn launch (&self, exec: String) {
    launch_orphan (&exec);
  }
}
