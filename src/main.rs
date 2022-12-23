use cache::DesktopEntryCache;
use std::io::Read;
use std::os::unix::net::UnixListener;
use std::process::Command;
use std::sync::{Arc, Mutex};
use x::Display;

mod app;
mod cache;
mod config;
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

use crate::config::Config;

struct CloseSocketOnExit;

impl Drop for CloseSocketOnExit {
  fn drop (&mut self) {
    close_socket ();
  }
}

fn close_socket () {
  std::fs::remove_file (common::SOCKET_PATH).unwrap ();
}

fn main () {
  if std::fs::metadata (common::SOCKET_PATH).is_ok () {
    eprintln! ("launcher-server: socket file already exists, unlinking it");
    Command::new ("unlink")
      .arg (common::SOCKET_PATH)
      .spawn ()
      .map (|mut p| p.wait ().ok ())
      .ok ();
  }
  let listener = match UnixListener::bind (common::SOCKET_PATH) {
    Ok (listener) => listener,
    Err (error) => {
      eprintln! ("launcher-server: failed to launch: {error}");
      return;
    }
  };
  let _closer = CloseSocketOnExit;
  let config = Config::load ();
  let cache = Arc::new (Mutex::new (DesktopEntryCache::new (&config.locale)));
  {
    let mut cache = cache.lock ().unwrap ();
    cache.rebuild ();
    if let Some (error) = cache.error () {
      eprintln! ("Failed to build desktop entry cache: {error}");
    }
  }
  x::init_threads ();
  input::set_locale_info ();
  let display = Display::connect (None);
  for stream in listener.incoming () {
    match stream {
      Ok (mut stream) => {
        let mut data = Vec::with_capacity (1);
        let mut stop = false;
        if stream.read_to_end (&mut data).is_ok () {
          for opcode in data {
            match opcode {
              common::OPCODE_SHOW => {
                println! ("launcher-server: show");
                App::new (display, cache.clone (), config.clone ()).run ();
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
