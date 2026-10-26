use crate::resource_monitor::observation::ProcessUser;
use std::{collections::HashMap, ptr};
use windows_sys::Win32::{
    Security::{GetLengthSid, IsValidSid},
    System::RemoteDesktop::{WTSEnumerateProcessesExW, WTSFreeMemoryExW,
        WTS_PROCESS_INFO_EXW, WTSTypeProcessInfoLevel1},
};

struct Processes { pointer: *mut WTS_PROCESS_INFO_EXW, count: u32 }

impl Drop for Processes {
    fn drop(&mut self) {
        unsafe { WTSFreeMemoryExW(WTSTypeProcessInfoLevel1, self.pointer.cast(), self.count); }
    }
}

pub fn enumerate(user_sid: &[u8]) -> Result<HashMap<u32, (String, ProcessUser)>, String> {
    let mut level = 1;
    let mut pointer = ptr::null_mut();
    let mut count = 0;
    // All sessions preserve the current-user scope when that user has several sessions.
    if unsafe { WTSEnumerateProcessesExW(ptr::null_mut(), &mut level, 0xffff_fffe, &mut pointer, &mut count) } == 0 {
        return Err(format!("无法枚举进程用户范围：{}", std::io::Error::last_os_error()));
    }
    let processes = Processes { pointer: pointer.cast(), count };
    if count == 0 { return Ok(HashMap::new()); }
    if processes.pointer.is_null() { return Err("进程用户范围缺少原生数据".into()); }
    let mut users = HashMap::new();
    for process in unsafe { std::slice::from_raw_parts(processes.pointer, count as usize) } {
        if process.pProcessName.is_null() { continue; }
        let length = (0..32_768).find(|index| unsafe { *process.pProcessName.add(*index) } == 0)
            .ok_or("进程名称超过原生读取边界")?;
        let name = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(process.pProcessName, length) });
        let user = if process.pUserSid.is_null() || unsafe { IsValidSid(process.pUserSid) } == 0 {
            ProcessUser::Unverified
        } else {
            let length = unsafe { GetLengthSid(process.pUserSid) } as usize;
            let sid = unsafe { std::slice::from_raw_parts(process.pUserSid.cast::<u8>(), length) };
            if sid == user_sid { ProcessUser::Current } else { ProcessUser::Other }
        };
        users.insert(process.ProcessId, (name, user));
    }
    Ok(users)
}
