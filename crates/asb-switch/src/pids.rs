//! Operating-system pid liveness probe (Windows).

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

#[cfg(not(windows))]
pub fn pid_liveness(_pid: u32) -> PidLiveness {
    PidLiveness::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_pid_is_alive() {
        let pid = std::process::id();
        assert_eq!(pid_liveness(pid), PidLiveness::Alive);
    }

    #[test]
    fn impossible_pid_is_dead() {
        // PID -1 is not a valid Windows process identifier; unlike a
        // recently exited child it cannot be reused between assertions.
        assert_eq!(pid_liveness(u32::MAX), PidLiveness::Dead);
    }
}
