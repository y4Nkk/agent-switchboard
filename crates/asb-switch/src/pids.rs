//! Operating-system PID liveness probe for supported desktop platforms.

use asb_core::PidLiveness;

#[cfg(windows)]
pub fn pid_liveness(pid: u32) -> PidLiveness {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if !handle.is_null() {
            CloseHandle(handle);
            return PidLiveness::Alive;
        }
        match GetLastError() {
            ERROR_ACCESS_DENIED => PidLiveness::Unknown,
            ERROR_INVALID_PARAMETER => PidLiveness::Dead,
            _ => PidLiveness::Dead,
        }
    }
}

#[cfg(unix)]
pub fn pid_liveness(pid: u32) -> PidLiveness {
    if pid == 0 {
        return PidLiveness::Dead;
    }
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return PidLiveness::Dead;
    };

    if unsafe { libc::kill(pid, 0) } == 0 {
        return PidLiveness::Alive;
    }

    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EPERM) => PidLiveness::Alive,
        Some(libc::ESRCH) => PidLiveness::Dead,
        _ => PidLiveness::Unknown,
    }
}

#[cfg(not(any(windows, unix)))]
compile_error!("Agent Switchboard supports only Windows, macOS, and Linux.");

#[test]
fn own_pid_is_alive() {
    let pid = std::process::id();
    assert_eq!(pid_liveness(pid), PidLiveness::Alive);
}

#[test]
fn impossible_pid_is_dead() {
    // This is not representable as a PID on supported platforms, so it
    // cannot be reused between assertions.
    assert_eq!(pid_liveness(u32::MAX), PidLiveness::Dead);
}
