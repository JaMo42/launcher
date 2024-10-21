use libc::{
    _exit, c_char, close, dup2, execl, fork, open, setsid, waitpid, O_RDWR, STDERR_FILENO,
    STDIN_FILENO, STDOUT_FILENO,
};
use std::{
    ffi::CString,
    io::Write,
    process::{Command, Stdio},
};

/// Launches and orphans the given command, making it a child of init and not
/// ourself. Any errors are ignored.
pub fn launch_orphan(command: &str) {
    unsafe {
        let pid = fork();
        let null = CString::new("/dev/null").unwrap();
        let null = open(null.as_ptr(), O_RDWR);
        if pid < 0 {
            return;
        }
        if pid == 0 {
            setsid();
            dup2(null, STDOUT_FILENO);
            dup2(null, STDERR_FILENO);
            dup2(null, STDIN_FILENO);
            let pid = fork();
            if pid < 0 {
                _exit(1)
            }
            if pid == 0 {
                let comm = CString::new(format!("bash -c '{}'", command)).unwrap();
                let path = CString::new("/bin/bash").unwrap();
                let arg0 = CString::new("bash").unwrap();
                let arg1 = CString::new("-c").unwrap();
                execl(
                    path.as_ptr(),
                    arg0.as_ptr(),
                    arg1.as_ptr(),
                    comm.as_ptr(),
                    std::ptr::null::<c_char>(),
                );
                close(null);
                _exit(1);
            }
            close(null);
            _exit(0)
        }
        close(null);
        let mut s = 0;
        waitpid(pid, &mut s, 0);
    }
}

pub fn copy(text: &str) {
    fn innner(text: &str) -> Result<(), std::io::Error> {
        let mut process = Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()?;
        process.stdin.as_mut().unwrap().write_all(text.as_bytes())?;
        process.wait()?;
        Ok(())
    }
    if let Err(error) = innner(text) {
        eprintln!("Failed to copy to clipboard: {}", error);
    }
}

pub fn paste() -> String {
    // Since reading the clipboard doesn't require launching a background
    // process we could do it ourselves, but it's still incredibly
    // convoluted and annoying, and we already depend on xclip.
    let output = match Command::new("xclip")
        .arg("-selection")
        .arg("clipboard")
        .arg("-o")
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Failed to read clipboard: {}", error);
            return String::new();
        }
    };
    String::from_utf8_lossy(&output.stdout).to_string()
}
