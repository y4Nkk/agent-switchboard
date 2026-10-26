use crate::resource_monitor::{observation::{Entry, Observation, ProcessKey, ProcessUser}, types::{push_issue, Issue}};
use std::{mem::{size_of, MaybeUninit}, path::PathBuf, time::Instant};

pub struct Reader { uid: libc::uid_t, timebase: libc::mach_timebase_info_data_t }
const PROC_UID_ONLY: u32 = 4;

impl Reader {
    pub fn new() -> Result<Self, String> {
        let mut timebase = libc::mach_timebase_info_data_t { numer: 0, denom: 0 };
        if unsafe { libc::mach_timebase_info(&mut timebase) } != 0 || timebase.denom == 0 {
            return Err("无法读取 Mach 计时单位".into());
        }
        Ok(Self { uid: unsafe { libc::geteuid() }, timebase })
    }

    pub fn entries(&self) -> Result<(Vec<Entry>, Vec<Issue>), String> {
        let (pids, truncated) = list_pids(self.uid)?;
        let mut entries = Vec::new();
        let mut issues = if truncated { vec![Issue::EnumerationRestricted] } else { Vec::new() };
        for pid in pids.into_iter().filter(|pid| *pid > 0) {
            match bsd_info(pid) {
                Ok(info) if info.pbi_uid == self.uid => entries.push(Entry { pid: info.pbi_pid,
                    parent_pid: info.pbi_ppid, name: process_name(&info), user: ProcessUser::Current }),
                Ok(_) => {}
                Err(Issue::ProcessExited) => {}
                Err(_) => push_issue(&mut issues, Issue::EnumerationRestricted),
            }
        }
        Ok((entries, issues))
    }

    pub fn read(&self, entry: &Entry) -> Result<Option<Observation>, Issue> {
        let pid = libc::pid_t::try_from(entry.pid).map_err(|_| Issue::IdentityUnavailable)?;
        let bsd = bsd_info(pid)?;
        if bsd.pbi_uid != self.uid { return Ok(None); }
        let birth = birth_stamp(&bsd);
        let task = task_info(pid);
        let at = Instant::now();
        let executable = executable_path(pid);
        let verified = bsd_info(pid)?;
        if verified.pbi_uid != self.uid { return Ok(None); }
        if birth_stamp(&verified) != birth { return Err(Issue::IdentityChanged); }
        let (cpu_ns, resident_bytes) = match task {
            Ok(task) => (cpu_nanoseconds(&task, &self.timebase),
                Ok(task.pti_resident_size)),
            Err(issue) => (Err(issue), Err(issue)),
        };
        Ok(Some(Observation { key: ProcessKey { pid: entry.pid, birth }, parent_pid: bsd.pbi_ppid,
            name: process_name(&bsd), executable, cpu_ns, resident_bytes, at }))
    }

    pub fn online_cpus(&self) -> Result<u32, String> {
        let count = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
        u32::try_from(count).ok().filter(|count| *count > 0).ok_or_else(|| "无法读取在线逻辑处理器数量".into())
    }
}

fn list_pids(uid: libc::uid_t) -> Result<(Vec<libc::pid_t>, bool), String> {
    for attempt in 0..3 {
        let bytes = unsafe { libc::proc_listpids(PROC_UID_ONLY, uid, std::ptr::null_mut(), 0) };
        if bytes <= 0 { return Err(format!("进程枚举失败：{}", std::io::Error::last_os_error())); }
        let capacity = bytes as usize / size_of::<libc::pid_t>() + 128;
        let mut pids = vec![0; capacity];
        let buffer_bytes = i32::try_from(pids.len() * size_of::<libc::pid_t>()).map_err(|_| "进程表过大")?;
        let read = unsafe { libc::proc_listpids(PROC_UID_ONLY, uid, pids.as_mut_ptr().cast(), buffer_bytes) };
        if read <= 0 { return Err(format!("进程表读取失败：{}", std::io::Error::last_os_error())); }
        let full = read >= buffer_bytes;
        pids.truncate(read as usize / size_of::<libc::pid_t>());
        if !full || attempt == 2 { return Ok((pids, full)); }
    }
    unreachable!()
}

fn bsd_info(pid: libc::pid_t) -> Result<libc::proc_bsdinfo, Issue> {
    let mut info = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = size_of::<libc::proc_bsdinfo>() as i32;
    let read = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, info.as_mut_ptr().cast(), size) };
    if read != size { return Err(last_issue()); }
    Ok(unsafe { info.assume_init() })
}

fn task_info(pid: libc::pid_t) -> Result<libc::proc_taskinfo, Issue> {
    let mut info = MaybeUninit::<libc::proc_taskinfo>::zeroed();
    let size = size_of::<libc::proc_taskinfo>() as i32;
    let read = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTASKINFO, 0, info.as_mut_ptr().cast(), size) };
    if read != size { return Err(last_issue()); }
    Ok(unsafe { info.assume_init() })
}

fn cpu_nanoseconds(task: &libc::proc_taskinfo, timebase: &libc::mach_timebase_info_data_t) -> Result<u64, Issue> {
    // PROC_PIDTASKINFO exposes Mach absolute ticks; Apple Silicon is not 1 ns/tick.
    let ticks = u128::from(task.pti_total_user) + u128::from(task.pti_total_system);
    u64::try_from(ticks * u128::from(timebase.numer) / u128::from(timebase.denom))
        .map_err(|_| Issue::CounterReset)
}

fn last_issue() -> Issue {
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ESRCH) => Issue::ProcessExited,
        Some(libc::EPERM) | Some(libc::EACCES) => Issue::AccessDenied,
        _ => Issue::CollectionFailed,
    }
}

fn birth_stamp(info: &libc::proc_bsdinfo) -> u128 {
    u128::from(info.pbi_start_tvsec) * 1_000_000 + u128::from(info.pbi_start_tvusec)
}

fn process_name(info: &libc::proc_bsdinfo) -> String {
    let name = if info.pbi_name[0] == 0 { &info.pbi_comm[..] } else { &info.pbi_name[..] };
    let bytes: Vec<u8> = name.iter().take_while(|byte| **byte != 0).map(|byte| *byte as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn executable_path(pid: libc::pid_t) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let mut bytes = vec![0u8; 4096];
    let count = unsafe { libc::proc_pidpath(pid, bytes.as_mut_ptr().cast(), bytes.len() as u32) };
    if count <= 0 { return None; }
    bytes.truncate(bytes.iter().position(|byte| *byte == 0)?);
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mach_ticks_use_the_platform_timebase() {
        let mut task = unsafe { MaybeUninit::<libc::proc_taskinfo>::zeroed().assume_init() };
        task.pti_total_user = 12_000_000;
        task.pti_total_system = 12_000_000;
        let arm = libc::mach_timebase_info_data_t { numer: 125, denom: 3 };
        let intel = libc::mach_timebase_info_data_t { numer: 1, denom: 1 };
        assert_eq!(cpu_nanoseconds(&task, &arm).unwrap(), 1_000_000_000);
        assert_eq!(cpu_nanoseconds(&task, &intel).unwrap(), 24_000_000);
    }
}
