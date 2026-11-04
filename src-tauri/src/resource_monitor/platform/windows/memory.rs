use crate::resource_monitor::types::Issue;
use std::{collections::HashMap, mem::size_of, ptr};
use windows_sys::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W,
    PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
};

struct Query(PDH_HQUERY);
impl Drop for Query { fn drop(&mut self) { unsafe { PdhCloseQuery(self.0); } } }

/// One collected Process-object snapshot; private resident pages exclude shared
/// WebView2 pages. PDH omits #N from array names, so names are not unique keys.
pub(super) fn private_working_sets() -> Result<HashMap<u32, u64>, Issue> {
    let mut query = ptr::null_mut();
    if unsafe { PdhOpenQueryW(ptr::null(), 0, &mut query) } != 0 { return Err(Issue::CollectionFailed); }
    let query = Query(query);
    let pids = add_counter(query.0, r"\Process(*)\ID Process")?;
    let private = add_counter(query.0, r"\Process(*)\Working Set - Private")?;
    if unsafe { PdhCollectQueryData(query.0) } != 0 { return Err(Issue::CollectionFailed); }
    join_snapshot(formatted_values(pids)?, formatted_values(private)?)
}

fn join_snapshot(owners: Vec<(String, Option<f64>)>, sizes: Vec<(String, Option<f64>)>) -> Result<HashMap<u32, u64>, Issue> {
    if owners.len() != sizes.len() { return Err(Issue::CollectionFailed); }
    let mut memory = HashMap::new();
    // Both counters enumerate the same collected object's instance positions.
    // Keep invalid slots to avoid shifting the following same-name process.
    for ((owner, pid), (instance, bytes)) in owners.into_iter().zip(sizes) {
        if owner != instance { return Err(Issue::CollectionFailed); }
        let Some(pid) = pid.filter(|value| value.is_finite() && value.fract() == 0.0
            && *value > 0.0 && *value <= f64::from(u32::MAX)) else { continue; };
        let Some(bytes) = bytes.filter(|value| value.is_finite() && value.fract() == 0.0
            && *value >= 0.0 && *value < u64::MAX as f64) else { continue; };
        if memory.insert(pid as u32, bytes as u64).is_some() { return Err(Issue::CollectionFailed); }
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

fn formatted_values(counter: PDH_HCOUNTER) -> Result<Vec<(String, Option<f64>)>, Issue> {
    let mut size = 0;
    let mut count = 0;
    let status = unsafe { PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, ptr::null_mut()) };
    if status != PDH_MORE_DATA { return Err(Issue::CollectionFailed); }
    let mut buffer = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
    let status = unsafe { PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, buffer.as_mut_ptr().cast()) };
    if status != 0 { return Err(Issue::CollectionFailed); }
    let items = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(), count as usize) };
    items.iter().map(|item| {
        let name = wide_name(item.szName).ok_or(Issue::CollectionFailed)?;
        let valid = item.FmtValue.CStatus == PDH_CSTATUS_VALID_DATA || item.FmtValue.CStatus == PDH_CSTATUS_NEW_DATA;
        Ok((name, valid.then(|| unsafe { item.FmtValue.Anonymous.doubleValue })))
    }).collect()
}

fn wide_name(name: *const u16) -> Option<String> {
    if name.is_null() { return None; }
    let mut length = 0;
    while length < 32_768 && unsafe { *name.add(length) } != 0 { length += 1; }
    if length == 32_768 { return None; }
    Some(String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(name, length) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(values: &[Option<f64>]) -> Vec<(String, Option<f64>)> {
        values.iter().map(|value| ("same-executable".into(), *value)).collect()
    }

    #[test]
    fn repeated_instance_names_keep_every_pid_and_real_zero() {
        let values = join_snapshot(rows(&[Some(11.0), Some(12.0), Some(13.0)]),
            rows(&[Some(1024.0), Some(2048.0), Some(0.0)])).unwrap();
        assert_eq!(values, HashMap::from([(11, 1024), (12, 2048), (13, 0)]));
    }

    #[test]
    fn invalid_slots_never_shift_the_next_same_name_process() {
        let values = join_snapshot(rows(&[Some(11.0), None, Some(13.0), Some(14.0)]),
            rows(&[None, Some(2048.0), Some(4096.0), Some(f64::NAN)])).unwrap();
        assert_eq!(values, HashMap::from([(13, 4096)]));
    }

    #[test]
    fn mismatched_instances_and_duplicate_pid_fail_instead_of_misattributing() {
        assert!(join_snapshot(rows(&[Some(11.0)]), rows(&[])).is_err());
        assert!(join_snapshot(rows(&[Some(11.0)]), vec![("other".into(), Some(1024.0))]).is_err());
        assert!(join_snapshot(rows(&[Some(11.0), Some(11.0)]), rows(&[Some(1.0), Some(2.0)])).is_err());
    }

    #[test]
    fn native_snapshot_keeps_two_live_instances_of_this_test_executable() {
        use std::{io::{BufRead, BufReader}, process::{Command, Stdio}};
        struct Child(std::process::Child);
        impl Drop for Child { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }
        let mut child = Child(Command::new(std::env::current_exe().unwrap()).args([
            "--exact", "resource_monitor::platform::windows::memory::tests::same_name_process_fixture",
            "--ignored", "--nocapture", "--test-threads=1",
        ]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap());
        let mut output = BufReader::new(child.0.stdout.take().unwrap());
        let mut line = String::new();
        while !line.contains("MEMORY_FIXTURE_READY") {
            line.clear();
            assert!(output.read_line(&mut line).unwrap() > 0, "fixture exited before readiness");
        }
        let memory = private_working_sets().unwrap();
        assert!(memory[&std::process::id()] > 0);
        assert!(memory[&child.0.id()] > 0);
    }

    #[test]
    #[ignore = "subprocess fixture owned by native_snapshot_keeps_two_live_instances_of_this_test_executable"]
    fn same_name_process_fixture() {
        use std::io::{Read, Write};
        println!("MEMORY_FIXTURE_READY");
        std::io::stdout().flush().unwrap();
        let _ = std::io::stdin().read(&mut [0]);
    }
}
