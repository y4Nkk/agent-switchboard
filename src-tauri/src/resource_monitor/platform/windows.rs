use crate::resource_monitor::{observation::{Entry, Observation, ProcessKey}, types::Issue};
use std::{mem::size_of, os::windows::ffi::OsStringExt, path::PathBuf, ptr, time::Instant};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER,
        ERROR_NO_MORE_FILES, FILETIME, HANDLE, INVALID_HANDLE_VALUE},
    Security::{GetLengthSid, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER},
    System::{
        Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
            PROCESSENTRY32W, TH32CS_SNAPPROCESS},
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
        Threading::{GetActiveProcessorCount, GetCurrentProcess, GetExitCodeProcess, GetProcessTimes,
            OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION},
    },
};

struct Handle(HANDLE);
impl Drop for Handle { fn drop(&mut self) { unsafe { CloseHandle(self.0); } } }

pub struct Reader { user_sid: Vec<u8> }

impl Reader {
    pub fn new() -> Result<Self, String> {
        let user_sid = token_sid(unsafe { GetCurrentProcess() })
            .map_err(|issue| format!("无法读取当前进程用户身份：{issue:?}"))?;
        Ok(Self { user_sid })
    }

    pub fn entries(&self) -> Result<(Vec<Entry>, Vec<Issue>), String> {
        let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if handle == INVALID_HANDLE_VALUE { return Err(format!("无法枚举进程：{}", std::io::Error::last_os_error())); }
        let snapshot = Handle(handle);
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut available = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        let mut entries = Vec::new();
        while available != 0 {
            let end = entry.szExeFile.iter().position(|character| *character == 0).unwrap_or(entry.szExeFile.len());
            entries.push(Entry { pid: entry.th32ProcessID, parent_pid: entry.th32ParentProcessID,
                name: String::from_utf16_lossy(&entry.szExeFile[..end]) });
            available = unsafe { Process32NextW(snapshot.0, &mut entry) };
        }
        if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
            return Err(format!("进程枚举未完成：{}", std::io::Error::last_os_error()));
        }
        Ok((entries, Vec::new()))
    }

    pub fn read(&self, entry: &Entry) -> Result<Option<Observation>, Issue> {
        let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.pid) };
        if raw.is_null() { return Err(last_issue()); }
        let handle = Handle(raw);
        if token_sid(handle.0)? != self.user_sid { return Ok(None); }
        let mut birth = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        if unsafe { GetProcessTimes(handle.0, &mut birth, &mut exit, &mut kernel, &mut user) } == 0 {
            return Err(last_issue());
        }
        let at = Instant::now();
        let birth = filetime(birth);
        if birth == 0 { return Err(Issue::IdentityUnavailable); }
        let cpu_ns = filetime(kernel).checked_add(filetime(user))
            .and_then(|time| time.checked_mul(100)).ok_or(Issue::CounterReset);
        let resident_bytes = resident_bytes(handle.0);
        let executable = executable_path(handle.0);
        let mut code = 0;
        if unsafe { GetExitCodeProcess(handle.0, &mut code) } == 0 { return Err(last_issue()); }
        if code != 259 { return Err(Issue::ProcessExited); }
        Ok(Some(Observation { key: ProcessKey { pid: entry.pid, birth: u128::from(birth) },
            parent_pid: entry.parent_pid, name: entry.name.clone(), executable, cpu_ns, resident_bytes, at }))
    }

    pub fn online_cpus(&self) -> Result<u32, String> {
        let count = unsafe { GetActiveProcessorCount(u16::MAX) };
        if count == 0 { Err("无法读取全部处理器组的在线逻辑处理器数量".into()) } else { Ok(count) }
    }
}

fn last_issue() -> Issue {
    match unsafe { GetLastError() } {
        ERROR_ACCESS_DENIED => Issue::AccessDenied,
        ERROR_INVALID_PARAMETER => Issue::ProcessExited,
        _ => Issue::CollectionFailed,
    }
}

fn filetime(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

pub fn process_key(pid: u32) -> Result<ProcessKey, Issue> {
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if raw.is_null() { return Err(last_issue()); }
    let handle = Handle(raw);
    let mut birth = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    if unsafe { GetProcessTimes(handle.0, &mut birth, &mut exit, &mut kernel, &mut user) } == 0 {
        return Err(last_issue());
    }
    let birth = filetime(birth);
    if birth == 0 { return Err(Issue::IdentityUnavailable); }
    Ok(ProcessKey { pid, birth: u128::from(birth) })
}

fn resident_bytes(handle: HANDLE) -> Result<u64, Issue> {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX { cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default() };
    if unsafe { GetProcessMemoryInfo(handle, ptr::from_mut(&mut counters).cast(), counters.cb) } == 0 {
        return Err(last_issue());
    }
    Ok(counters.WorkingSetSize as u64)
}

fn executable_path(handle: HANDLE) -> Option<PathBuf> {
    let mut buffer = vec![0u16; 32_768];
    let mut size = buffer.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) } == 0 { return None; }
    Some(PathBuf::from(std::ffi::OsString::from_wide(&buffer[..size as usize])))
}

fn token_sid(process: HANDLE) -> Result<Vec<u8>, Issue> {
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 { return Err(last_issue()); }
    let token = Handle(token);
    let mut needed = 0;
    unsafe { GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut needed); }
    if needed == 0 || needed > 65_536 { return Err(Issue::IdentityUnavailable); }
    // TOKEN_USER contains pointers; word allocation preserves their alignment.
    let mut buffer = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    if unsafe { GetTokenInformation(token.0, TokenUser, buffer.as_mut_ptr().cast(), needed, &mut needed) } == 0 {
        return Err(last_issue());
    }
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let length = unsafe { GetLengthSid(user.User.Sid) } as usize;
    if length == 0 { return Err(Issue::IdentityUnavailable); }
    Ok(unsafe { std::slice::from_raw_parts(user.User.Sid.cast::<u8>(), length) }.to_vec())
}
