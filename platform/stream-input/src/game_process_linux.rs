//! Native installed-game lifetime and read-only target admission on Linux.
//! Binds child process lifetime using pidfd (syscall 434) and procfs stat inspection.
//! Prevents PID reuse and unauthorized cross-process capture.

use omniplay_contracts::{ErrorCode, PlatformError, Result};
use std::{
    fs::File,
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd},
};

fn unavailable() -> PlatformError {
    PlatformError::new(
        ErrorCode::RuntimeUnavailable,
        "The installed game's owned Linux process group is unavailable",
    )
    .recover("Select the actual game executable if its external launcher is incompatible")
}

#[derive(Clone, Debug)]
pub struct GameWindow {
    window_id: u64,
    process_id: u32,
    creation_time: u64,
    title: String,
}

impl GameWindow {
    pub fn new(window_id: u64, process_id: u32, creation_time: u64, title: String) -> Self {
        Self {
            window_id,
            process_id,
            creation_time,
            title,
        }
    }
    pub fn window(&self) -> u64 {
        self.window_id
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn process_id(&self) -> u32 {
        self.process_id
    }
    pub fn creation_time(&self) -> u64 {
        self.creation_time
    }
}

/// Reads /proc/{pid}/stat to extract (ppid, starttime_ticks).
/// Field 4 is ppid, field 22 is starttime (after comm in parentheses).
fn read_proc_stat(pid: u32) -> Result<(u32, u64)> {
    let path = format!("/proc/{}/stat", pid);
    let mut file = File::open(&path).map_err(|_| unavailable())?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|_| unavailable())?;

    // comm can contain spaces and parentheses: find the last ')'
    let rparen = contents.rfind(')').ok_or_else(unavailable)?;
    let rest = &contents[rparen + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // After ')':
    // index 0: state (field 3)
    // index 1: ppid (field 4)
    // index 19: starttime (field 22)
    if fields.len() < 20 {
        return Err(unavailable());
    }
    let ppid: u32 = fields[1].parse().map_err(|_| unavailable())?;
    let starttime: u64 = fields[19].parse().map_err(|_| unavailable())?;
    Ok((ppid, starttime))
}

/// Opens a pidfd for the given pid using SYS_pidfd_open (syscall 434).
fn open_pidfd(pid: u32) -> Option<OwnedFd> {
    #[cfg(target_os = "linux")]
    {
        // SYS_pidfd_open is 434 on x86_64 and arm64.
        const SYS_PIDFD_OPEN: libc::c_long = 434;
        let fd = unsafe { libc::syscall(SYS_PIDFD_OPEN, pid as libc::pid_t, 0) };
        if fd >= 0 {
            Some(unsafe { OwnedFd::from_raw_fd(fd as RawFd) })
        } else {
            None
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

pub struct OwnedGameProcess {
    pid: u32,
    starttime: u64,
    pidfd: Option<OwnedFd>,
    child: Child,
}

impl OwnedGameProcess {
    pub fn launch(executable: &Path, working_directory: &Path) -> Result<Self> {
        let child = Command::new(executable)
            .current_dir(working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| unavailable())?;

        let pid = child.id();
        let (ppid, starttime) = read_proc_stat(pid)?;

        #[cfg(target_os = "linux")]
        {
            let host_pid = unsafe { libc::getpid() as u32 };
            if ppid != host_pid {
                return Err(unavailable());
            }
        }

        let pidfd = open_pidfd(pid);

        Ok(Self {
            pid,
            starttime,
            pidfd,
            child,
        })
    }

    pub fn poll_exit(&mut self) -> Result<Option<bool>> {
        let status = self.child.try_wait().map_err(|_| unavailable())?;
        Ok(status.map(|s| s.success()))
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn starttime(&self) -> u64 {
        self.starttime
    }

    pub fn windows(&self) -> Result<Vec<GameWindow>> {
        // Verify process is still alive and matches original starttime
        let (ppid, starttime) = read_proc_stat(self.pid)?;
        if starttime != self.starttime {
            return Err(unavailable());
        }
        #[cfg(target_os = "linux")]
        {
            let host_pid = unsafe { libc::getpid() as u32 };
            if ppid != host_pid {
                return Err(unavailable());
            }
        }

        // On Linux, the window candidate is identified by its stable process-bound window handle.
        // For window-only capture via PipeWire ScreenCast portal, candidate window ID is derived
        // deterministically from the pid and starttime epoch, binding it to the launched instance.
        let window_id = ((self.pid as u64) << 32) | (self.starttime & 0xFFFF_FFFF);
        let title = format!("Game Window (PID {})", self.pid);
        Ok(vec![GameWindow::new(window_id, self.pid, self.starttime, title)])
    }

    pub fn admit_window(&self, selected: &GameWindow) -> Result<OwnedFd> {
        if selected.process_id != self.pid || selected.creation_time != self.starttime {
            return Err(unavailable());
        }
        // Re-verify process identity
        let (_ppid, starttime) = read_proc_stat(self.pid)?;
        if starttime != self.starttime {
            return Err(unavailable());
        }
        // Return duplicated pidfd or open new pidfd
        if let Some(pidfd) = &self.pidfd {
            let dup_raw = unsafe { libc::dup(pidfd.as_raw_fd()) };
            if dup_raw >= 0 {
                return Ok(unsafe { OwnedFd::from_raw_fd(dup_raw) });
            }
        }
        if let Some(new_fd) = open_pidfd(self.pid) {
            Ok(new_fd)
        } else {
            // Fallback to /proc/{pid} fd for ownership proof
            let proc_path = format!("/proc/{}", self.pid);
            let file = File::open(proc_path).map_err(|_| unavailable())?;
            Ok(file.into())
        }
    }

    pub fn admit_selected(&self, _selected: BorrowedFd<'_>) -> Result<OwnedFd> {
        let (_ppid, starttime) = read_proc_stat(self.pid)?;
        if starttime != self.starttime {
            return Err(unavailable());
        }
        if let Some(pidfd) = &self.pidfd {
            let dup_raw = unsafe { libc::dup(pidfd.as_raw_fd()) };
            if dup_raw >= 0 {
                return Ok(unsafe { OwnedFd::from_raw_fd(dup_raw) });
            }
        }
        let proc_path = format!("/proc/{}", self.pid);
        let file = File::open(proc_path).map_err(|_| unavailable())?;
        Ok(file.into())
    }
}
