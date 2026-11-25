//! Rune's diagnostics go to stderr, which an Android app has no reader for: send stdout and
//! stderr to logcat, a line at a time, under the `rune` tag.

#![allow(unsafe_code)]

use std::ffi::CString;
use std::io::{BufRead, BufReader};
use std::os::fd::FromRawFd;
use std::sync::Once;

pub(super) fn redirect_stdio() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let mut fds = [0; 2];
        // SAFETY: `fds` holds the two descriptors `pipe` writes.
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 { return }
        // SAFETY: `fds[1]` is the pipe's open write end; stdout and stderr now write into it.
        unsafe {
            libc::dup2(fds[1], libc::STDOUT_FILENO);
            libc::dup2(fds[1], libc::STDERR_FILENO);
            libc::close(fds[1]);
        }
        // SAFETY: `fds[0]` is the pipe's read end, owned by this file from here on.
        let read = unsafe { std::fs::File::from_raw_fd(fds[0]) };
        std::thread::Builder::new().name("rune-logcat".into()).spawn(move || {
            for line in BufReader::new(read).lines().map_while(Result::ok) {
                let Ok(text) = CString::new(line) else { continue };
                // SAFETY: both strings are NUL-terminated and live for the call.
                unsafe { ndk_sys::__android_log_write(ndk_sys::android_LogPriority::ANDROID_LOG_INFO.0 as i32, c"rune".as_ptr(), text.as_ptr()) };
            }
        }).ok();
    });
}
