use libc::{_exit, c_char, execl, fork, setsid, waitpid};
use std::ffi::CString;

/// Launches and orphans the given command, making a child or init and not
/// ourself. Any errors are ignored.
pub fn launch_orphan (command: &str) {
  unsafe {
    let pid = fork ();
    if pid < 0 {
      return;
    }
    if pid == 0 {
      setsid ();
      let pid = fork ();
      if pid < 0 {
        _exit (1)
      }
      if pid == 0 {
        let comm = CString::new (format! ("bash -c '{}'", command)).unwrap ();
        let path = CString::new ("/bin/bash").unwrap ();
        let arg0 = CString::new ("bash").unwrap ();
        let arg1 = CString::new ("-c").unwrap ();
        execl (
          path.as_ptr (),
          arg0.as_ptr (),
          arg1.as_ptr (),
          comm.as_ptr (),
          std::ptr::null::<c_char> (),
        );
        _exit (1);
      }
      _exit (0)
    }
    let mut s = 0;
    waitpid (pid, &mut s, 0);
  }
}
