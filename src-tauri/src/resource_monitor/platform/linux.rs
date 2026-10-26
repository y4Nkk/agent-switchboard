use crate::resource_monitor::{observation::{Entry, Observation, ProcessKey, ProcessUser}, types::{push_issue, Issue}};
use std::{fs, io::Read, path::PathBuf, time::Instant};

pub struct Reader { uid: u32, ticks: u64, page_size: u64 }

struct Stat { name: String, parent: u32, birth: u64, user: u64, system: u64, rss: u64 }

impl Reader {
    pub fn new() -> Result<Self, String> {
        let local = fs::read_to_string("/proc/self/stat").map_err(|error| format!("无法核实当前 PID 命名空间：{error}"))?;
        if local.split_whitespace().next().and_then(|pid| pid.parse::<u32>().ok()) != Some(std::process::id()) {
            return Err("/proc 与当前应用的 PID 命名空间不一致，无法核实进程归属".into());
        }
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if ticks <= 0 || page_size <= 0 { return Err("无法读取进程计时单位或系统页大小".into()); }
        Ok(Self { uid: unsafe { libc::geteuid() }, ticks: ticks as u64, page_size: page_size as u64 })
    }

    pub fn entries(&self) -> Result<(Vec<Entry>, Vec<Issue>), String> {
        let directories = fs::read_dir("/proc").map_err(|error| format!("无法枚举 /proc：{error}"))?;
        let mut entries = Vec::new();
        let mut issues = namespace_issues();
        for directory in directories {
            let directory = directory.map_err(|error| format!("进程目录读取失败：{error}"))?;
            let Some(pid) = directory.file_name().to_str().and_then(|name| name.parse::<u32>().ok()) else { continue; };
            match effective_uid(pid) {
                Ok(uid) if uid != self.uid => continue,
                Ok(_) => {}
                Err(Issue::ProcessExited) => continue,
                Err(_) => { push_issue(&mut issues, Issue::EnumerationRestricted); continue; }
            }
            match read_stat(pid) {
                Ok(stat) => entries.push(Entry { pid, parent_pid: stat.parent, name: stat.name, user: ProcessUser::Current }),
                Err(Issue::ProcessExited) => {}
                Err(_) => push_issue(&mut issues, Issue::EnumerationRestricted),
            }
        }
        Ok((entries, issues))
    }

    pub fn read(&self, entry: &Entry) -> Result<Option<Observation>, Issue> {
        let root = PathBuf::from(format!("/proc/{}", entry.pid));
        if effective_uid(entry.pid)? != self.uid { return Ok(None); }
        let stat = read_stat(entry.pid)?;
        let at = Instant::now();
        let executable = fs::read_link(root.join("exe")).ok();
        let verified = read_stat(entry.pid)?;
        if effective_uid(entry.pid)? != self.uid { return Ok(None); }
        if stat.birth != verified.birth { return Err(Issue::IdentityChanged); }
        let cpu_ns = stat.user.checked_add(stat.system)
            .and_then(|ticks| u64::try_from(u128::from(ticks) * 1_000_000_000 / u128::from(self.ticks)).ok())
            .ok_or(Issue::CounterReset);
        Ok(Some(Observation { key: ProcessKey { pid: entry.pid, birth: u128::from(stat.birth) },
            parent_pid: stat.parent, name: stat.name, executable, cpu_ns,
            resident_bytes: stat.rss.checked_mul(self.page_size).ok_or(Issue::CounterReset), at }))
    }

    pub fn online_cpus(&self) -> Result<u32, String> {
        let count = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
        u32::try_from(count).ok().filter(|count| *count > 0).ok_or_else(|| "无法读取在线逻辑处理器数量".into())
    }
}

fn io_issue(error: &std::io::Error) -> Issue {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => Issue::AccessDenied,
        std::io::ErrorKind::NotFound => Issue::ProcessExited,
        _ => Issue::CollectionFailed,
    }
}

fn read_stat(pid: u32) -> Result<Stat, Issue> {
    let bytes = fs::read(format!("/proc/{pid}/stat")).map_err(|error| io_issue(&error))?;
    parse_stat(&String::from_utf8_lossy(&bytes)).ok_or(Issue::IdentityUnavailable)
}

fn effective_uid(pid: u32) -> Result<u32, Issue> {
    let file = fs::File::open(format!("/proc/{pid}/status")).map_err(|error| io_issue(&error))?;
    let mut status = Vec::new();
    // Uid precedes variable-size groups and memory details in the kernel's status header.
    file.take(4096).read_to_end(&mut status).map_err(|error| io_issue(&error))?;
    parse_uid(&status).ok_or(Issue::IdentityUnavailable)
}

fn parse_uid(status: &[u8]) -> Option<u32> {
    let line = status.split(|byte| *byte == b'\n').find(|line| line.starts_with(b"Uid:"))?;
    let effective = line[4..].split(|byte| byte.is_ascii_whitespace()).filter(|field| !field.is_empty()).nth(1)?;
    std::str::from_utf8(effective).ok()?.parse().ok()
}

fn parse_stat(text: &str) -> Option<Stat> {
    let start = text.find('(')?;
    let end = text.rfind(')')?;
    if end <= start { return None; }
    let fields: Vec<&str> = text.get(end + 1..)?.split_whitespace().collect();
    let number = |index: usize| fields.get(index)?.parse::<u64>().ok();
    Some(Stat { name: text.get(start + 1..end)?.to_string(), parent: u32::try_from(number(1)?).ok()?,
        user: number(11)?, system: number(12)?, birth: number(19)?, rss: number(21)? })
}

fn namespace_issues() -> Vec<Issue> {
    let nested = fs::read_to_string("/proc/self/status").ok().is_some_and(|text| text.lines()
        .find(|line| line.starts_with("NSpid:")).is_some_and(|line| line.split_whitespace().count() > 2));
    if nested { vec![Issue::NamespaceBoundary] } else { Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::{parse_stat, parse_uid};

    #[test]
    fn stat_names_may_contain_spaces_and_parentheses() {
        let text = "42 (test (child)) S 7 0 0 0 0 0 0 0 0 0 11 12 0 0 0 0 0 0 33 0 44";
        let stat = parse_stat(text).unwrap();
        assert_eq!((stat.parent, stat.user, stat.system, stat.birth, stat.rss), (7, 11, 12, 33, 44));
    }

    #[test]
    fn effective_user_is_independent_of_real_user_and_non_utf8_process_names() {
        assert_eq!(parse_uid(b"Name:\t\xff\nUid:\t0\t1001\t1002\t1003\n"), Some(1001));
        assert_eq!(parse_uid(b"Uid:\t1001\t0\t1001\t1001\n"), Some(0));
        assert_eq!(parse_uid(b"Uid:\t1001\n"), None);
    }
}
