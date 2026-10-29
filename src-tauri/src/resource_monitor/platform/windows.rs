mod users;

use crate::resource_monitor::{observation::{Entry, Observation, ProcessKey, ProcessUser}, types::Issue};
use std::{collections::HashMap, mem::size_of, os::windows::ffi::OsStringExt, path::PathBuf, ptr,
    time::Instant};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER,
        ERROR_NO_MORE_FILES, FILETIME, HANDLE, INVALID_HANDLE_VALUE},
    Security::{GetLengthSid, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER},
    System::{
        Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
            PROCESSENTRY32W, TH32CS_SNAPPROCESS},
        Performance::{PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
            PdhGetFormattedCounterArrayW, PdhOpenQueryW, PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA,
            PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA},
        Threading::{GetActiveProcessorCount, GetCurrentProcess, GetExitCodeProcess, GetProcessTimes,
            OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION},
    },
};

struct Handle(HANDLE);
impl Drop for Handle { fn drop(&mut self) { unsafe { CloseHandle(self.0); } } }

struct Query(PDH_HQUERY);
impl Drop for Query { fn drop(&mut self) { unsafe { PdhCloseQuery(self.0); } } }

pub struct Reader { user_sid: Vec<u8>, memory: Result<HashMap<u32, u64>, Issue> }

impl Reader {
    pub fn new() -> Result<Self, String> {
        let user_sid = token_sid(unsafe { GetCurrentProcess() })
            .map_err(|issue| format!("无法读取当前进程用户身份：{issue:?}"))?;
        let memory = private_working_sets();
        Ok(Self { user_sid, memory })
    }

    pub fn entries(&self) -> Result<(Vec<Entry>, Vec<Issue>), String> {
        let users = users::enumerate(&self.user_sid)?;
        let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if handle == INVALID_HANDLE_VALUE { return Err(format!("无法枚举进程：{}", std::io::Error::last_os_error())); }
        let snapshot = Handle(handle);
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut available = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        let mut entries = Vec::new();
        while available != 0 {
            let end = entry.szExeFile.iter().position(|character| *character == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            let user = users.get(&entry.th32ProcessID).filter(|(observed_name, _)|
                observed_name.eq_ignore_ascii_case(&name)).map(|(_, user)| *user).unwrap_or(ProcessUser::Unverified);
            entries.push(Entry { pid: entry.th32ProcessID, parent_pid: entry.th32ParentProcessID, name, user });
            available = unsafe { Process32NextW(snapshot.0, &mut entry) };
        }
        if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
            return Err(format!("进程枚举未完成：{}", std::io::Error::last_os_error()));
        }
        Ok((entries, Vec::new()))
    }

    pub fn read(&self, entry: &Entry) -> Result<Option<Observation>, Issue> {
        match entry.user {
            ProcessUser::Other => return Ok(None),
            ProcessUser::Unverified => return Err(Issue::EnumerationRestricted),
            ProcessUser::Current => {}
        }
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
        let resident_bytes = match &self.memory {
            Ok(memory) => memory.get(&entry.pid).copied().ok_or(Issue::ProcessExited),
            Err(issue) => Err(*issue),
        };
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

/// One PDH snapshot of every process's private working set: the resident pages
/// a process owns exclusively, matching Task Manager's "Memory" column. Summed
/// across processes this does not double-count the pages WebView2 children share.
fn private_working_sets() -> Result<HashMap<u32, u64>, Issue> {
    let mut query = ptr::null_mut();
    if unsafe { PdhOpenQueryW(ptr::null(), 0, &mut query) } != 0 { return Err(Issue::CollectionFailed); }
    let query = Query(query);
    let pids = add_counter(query.0, r"\Process(*)\ID Process")?;
    let private = add_counter(query.0, r"\Process(*)\Working Set - Private")?;
    if unsafe { PdhCollectQueryData(query.0) } != 0 { return Err(Issue::CollectionFailed); }
    let owners: HashMap<String, u32> = formatted_values(pids)?.into_iter().filter_map(|(name, value)|
        u32::try_from(value as i64).ok().map(|pid| (name, pid))).collect();
    let mut memory = HashMap::new();
    for (name, value) in formatted_values(private)? {
        if let Some(pid) = owners.get(&name) { memory.insert(*pid, value.max(0.0) as u64); }
    }
    Ok(memory)
}

fn add_counter(query: PDH_HQUERY, path: &str) -> Result<PDH_HCOUNTER, Issue> {
    let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
    let mut counter = ptr::null_mut();
    if unsafe { PdhAddEnglishCounterW(query, wide.as_ptr(), 0, &mut counter) } != 0 {
        return Err(Issue::CollectionFailed);
    }
    Ok(counter)
}

fn formatted_values(counter: PDH_HCOUNTER) -> Result<Vec<(String, f64)>, Issue> {
    let mut size = 0;
    let mut count = 0;
    let status = unsafe { PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, ptr::null_mut()) };
    if status != PDH_MORE_DATA { return Err(Issue::CollectionFailed); }
    // Counter values carry an f64, so the byte buffer must be 8-byte aligned.
    let mut buffer = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
    let status = unsafe { PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, buffer.as_mut_ptr().cast()) };
    if status != 0 { return Err(Issue::CollectionFailed); }
    let items = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(), count as usize) };
    Ok(items.iter().filter_map(|item| {
        if item.FmtValue.CStatus != PDH_CSTATUS_VALID_DATA && item.FmtValue.CStatus != PDH_CSTATUS_NEW_DATA { return None; }
        Some((wide_name(item.szName)?, unsafe { item.FmtValue.Anonymous.doubleValue }))
    }).collect())
}

fn wide_name(name: *const u16) -> Option<String> {
    if name.is_null() { return None; }
    let mut length = 0;
    while length < 32_768 && unsafe { *name.add(length) } != 0 { length += 1; }
    if unsafe { *name.add(length) } != 0 { return None; }
    Some(String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(name, length) }))
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
