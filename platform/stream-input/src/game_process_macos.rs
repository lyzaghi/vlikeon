//! Native installed-game lifetime and read-only target admission on macOS.
//! Uses proc_pidinfo to inspect process BSD info (PID, PPID, start time tv_sec/tv_usec, flags).
//! Enforces exact host child process ownership and prevents PID reuse.

use omniplay_contracts::{ErrorCode, PlatformError, Result};
use std::{
    path::Path,
    process::{Child, Command, Stdio},
};

fn unavailable() -> PlatformError {
    PlatformError::new(
        ErrorCode::RuntimeUnavailable,
        "The installed game's owned macOS process group is unavailable",
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

/// Representation of macOS proc_bsdinfo from sys/proc_info.h
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ProcBsdInfo {
    pbi_flags: u32,
    pbi_status: u32,
    pbi_xstatus: u32,
    pbi_pid: u32,
    pbi_ppid: u32,
    pbi_uid: u32,
    pbi_gid: u32,
    pbi_ruid: u32,
    pbi_rgid: u32,
    pbi_svuid: u32,
    pbi_svgid: u32,
    rfu_1: u32,
    pbi_comm: [u8; 16],
    pbi_name: [u8; 32],
    pbi_nfiles: u32,
    pbi_pgid: u32,
    pbi_pjobc: u32,
    pbi_e_tdev: u32,
    pbi_e_tpgid: u32,
    pbi_nice: i32,
    pbi_start_tvsec: u64,
    pbi_start_tvusec: u64,
}

const PROC_PIDTBSDINFO: libc::c_int = 3;
const PROC_FLAG_INEXIT: u32 = 0x00000001;
const PROC_FLAG_ZOMBIE: u32 = 0x00000002;

fn inspect_proc_bsdinfo(pid: u32) -> Result<ProcBsdInfo> {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn proc_pidinfo(
                pid: libc::c_int,
                flavor: libc::c_int,
                arg: u64,
                buffer: *mut libc::c_void,
                buffersize: libc::c_int,
            ) -> libc::c_int;
        }

        let mut info = ProcBsdInfo::default();
        let size = std::mem::size_of::<ProcBsdInfo>() as libc::c_int;
        let ret = unsafe {
            proc_pidinfo(
                pid as libc::c_int,
                PROC_PIDTBSDINFO,
                0,
                &mut info as *mut _ as *mut libc::c_void,
                size,
            )
        };
        if ret != size {
            return Err(unavailable());
        }
        if (info.pbi_flags & (PROC_FLAG_INEXIT | PROC_FLAG_ZOMBIE)) != 0 {
            return Err(unavailable());
        }
        Ok(info)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        Err(unavailable())
    }
}

pub struct OwnedGameProcess {
    pid: u32,
    start_sec: u64,
    start_usec: u64,
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
        let info = inspect_proc_bsdinfo(pid)?;

        #[cfg(target_os = "macos")]
        {
            let host_pid = unsafe { libc::getpid() as u32 };
            if info.pbi_ppid != host_pid {
                return Err(unavailable());
            }
        }

        Ok(Self {
            pid,
            start_sec: info.pbi_start_tvsec,
            start_usec: info.pbi_start_tvusec,
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

    pub fn creation_time(&self) -> u64 {
        self.start_sec.saturating_mul(1_000_000).saturating_add(self.start_usec)
    }

    pub fn windows(&self) -> Result<Vec<GameWindow>> {
        let info = inspect_proc_bsdinfo(self.pid)?;
        if info.pbi_start_tvsec != self.start_sec || info.pbi_start_tvusec != self.start_usec {
            return Err(unavailable());
        }

        let creation = self.creation_time();
        let window_id = ((self.pid as u64) << 32) | (creation & 0xFFFF_FFFF);
        let title = format!("macOS Game Window (PID {})", self.pid);
        Ok(vec![GameWindow::new(window_id, self.pid, creation, title)])
    }

    pub fn admit_window(&self, selected: &GameWindow) -> Result<u32> {
        if selected.process_id != self.pid || selected.creation_time != self.creation_time() {
            return Err(unavailable());
        }
        let info = inspect_proc_bsdinfo(self.pid)?;
        if info.pbi_start_tvsec != self.start_sec || info.pbi_start_tvusec != self.start_usec {
            return Err(unavailable());
        }
        Ok(self.pid)
    }

    pub fn admit_selected(&self, pid: u32) -> Result<u32> {
        if pid != self.pid {
            return Err(unavailable());
        }
        let info = inspect_proc_bsdinfo(self.pid)?;
        if info.pbi_start_tvsec != self.start_sec || info.pbi_start_tvusec != self.start_usec {
            return Err(unavailable());
        }
        Ok(self.pid)
    }
}
