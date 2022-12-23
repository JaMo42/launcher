use cache::DesktopEntryCache;
use search::sort_search_results;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::UnixListener;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

mod app;
mod cache;
mod draw;
mod entry;
mod history;
mod input;
mod layout;
mod list_view;
mod res;
mod search;
mod ui;
mod util;
mod x;

use app::App;

struct CloseSocketOnExit;

impl Drop for CloseSocketOnExit {
  fn drop (&mut self) {
    close_socket ();
  }
}

fn close_socket () {
  std::fs::remove_file (common::SOCKET_PATH).unwrap ();
}

fn _main () {
  let listener = match UnixListener::bind (common::SOCKET_PATH) {
    Ok (listener) => listener,
    Err (_) => {
      eprintln! ("launcher-server: server already running");
      return;
    }
  };
  let _closer = CloseSocketOnExit;
  for stream in listener.incoming () {
    match stream {
      Ok (mut stream) => {
        let mut data = Vec::with_capacity (1);
        let mut stop = false;
        if stream.read_to_end (&mut data).is_ok () {
          for opcode in data {
            match opcode {
              common::OPCODE_SHOW => {
                println! ("launcher-server: Hello World");
              }
              common::OPCODE_STOP => {
                println! ("launcher-server: stop");
                stop = true;
                break;
              }
              common::OPCODE_REBUILD_CACHE => {
                println! ("launcher-server: rebuild cache");
              }
              _ => {
                eprintln! ("launcher-server: error: invalid operation: {}", opcode);
              }
            }
          }
        }
        if stop {
          break;
        }
      }
      Err (error) => {
        eprintln! ("launcher-server: socket error: {error}");
        break;
      }
    }
  }
}

fn main () {
  let cache = Arc::new (Mutex::new (DesktopEntryCache::new (None)));
  {
    let mut cache = cache.lock ().unwrap ();
    cache.rebuild ();
    if let Some (error) = cache.error () {
      eprintln! ("Failed to build desktop entry cache: {error}");
    }
  }
  let history = Rc::new (RefCell::new (HashMap::new ()));

  input::set_locale_info ();
  App::new (cache.clone (), history.clone ()).run ();
  println! ("Good bye");
}
