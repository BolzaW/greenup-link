#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
#[cfg(unix)]
use std::path::{Path, PathBuf};

#[cfg(unix)]
pub struct PtyPair {
    pub master: OwnedFd,
    pub slave: OwnedFd,
    pub slave_path: PathBuf,
    pub symlink_path: Option<PathBuf>,
}

#[cfg(unix)]
impl PtyPair {
    pub fn new(symlink: Option<&Path>) -> std::io::Result<Self> {
        let mut master_fd: libc::c_int = -1;
        let mut slave_fd: libc::c_int = -1;

        let res = unsafe {
            libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };

        if res != 0 {
            return Err(std::io::Error::last_os_error());
        }

        let master = unsafe { OwnedFd::from_raw_fd(master_fd) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave_fd) };

        // Retrieve the slave PTY device path (e.g. /dev/pts/3)
        let slave_path = unsafe {
            let ptr = libc::ptsname(master.as_raw_fd());
            if ptr.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let cstr = std::ffi::CStr::from_ptr(ptr);
            PathBuf::from(cstr.to_string_lossy().into_owned())
        };

        // Configure raw mode on the PTY so bytes pass through unbuffered
        // (disable echo, disable canonical mode, disable carriage-return translation)
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(master.as_raw_fd(), &mut termios) == 0 {
                libc::cfmakeraw(&mut termios);
                libc::tcsetattr(master.as_raw_fd(), libc::TCSANOW, &termios);
            }
            if libc::tcgetattr(slave.as_raw_fd(), &mut termios) == 0 {
                libc::cfmakeraw(&mut termios);
                libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &termios);
            }
        }

        let mut symlink_path = None;
        if let Some(link) = symlink {
            if link.exists() || fs::symlink_metadata(link).is_ok() {
                let _ = fs::remove_file(link);
            }
            std::os::unix::fs::symlink(&slave_path, link)?;
            symlink_path = Some(link.to_path_buf());
        }

        Ok(Self {
            master,
            slave,
            slave_path,
            symlink_path,
        })
    }
}

#[cfg(unix)]
impl Drop for PtyPair {
    fn drop(&mut self) {
        if let Some(ref link) = self.symlink_path {
            if link.exists() || fs::symlink_metadata(link).is_ok() {
                let _ = fs::remove_file(link);
            }
        }
    }
}

#[cfg(not(unix))]
pub struct PtyPair;

#[cfg(not(unix))]
impl PtyPair {
    pub fn new(_symlink: Option<&std::path::Path>) -> std::io::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "PTY simulation is only supported on Linux / WSL / Unix systems",
        ))
    }
}
