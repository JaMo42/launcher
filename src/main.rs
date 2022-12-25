use cache::DesktopEntryCache;
use single_instance::SingleInstance;
use std::{
  sync::{Arc, Mutex},
  time::Instant,
};
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
use config::Config;

fn main () {
  let instance_guard = SingleInstance::new ("com.github.JaMo42.launcher").unwrap ();
  if !instance_guard.is_single () {
    println! ("Already running");
    return;
  }
  let config = Config::load ();
  let cache = Arc::new (Mutex::new (DesktopEntryCache::new (&config.locale)));
  {
    let mut cache = cache.lock ().unwrap ();
    let time = Instant::now ();
    cache.rebuild ();
    let elapsed = time.elapsed ();
    if let Some (error) = cache.error () {
      eprintln! ("Failed to build desktop entry cache: {error}");
    } else {
      println! (
        "Built desktop entry cache in {} milliseconds",
        elapsed.as_millis ()
      );
    }
  }
  x::init_threads ();
  input::set_locale_info ();
  let mut display = Display::connect (None);
  App::new (display, cache.clone (), config.clone ()).run ();
  display.close ();
}
