use super::{discovery, observation::InterfaceProcesses, types::{Group, InterfaceCoverage}};
use std::{collections::HashMap, fs, path::{Path, PathBuf}, process::{Child, Command, Stdio}, thread, time::{Duration, Instant}};

const FIXTURE_TEST: &str = "resource_monitor::process_tree_tests::fixture_process";

struct Children { stop: PathBuf, processes: Vec<Child> }

impl Drop for Children {
    fn drop(&mut self) {
        let _ = fs::write(&self.stop, b"stop");
        for child in &mut self.processes {
            let deadline = Instant::now() + Duration::from_secs(5);
            while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn command(executable: &Path, directory: &Path, branch: &str, role: &str) -> Command {
    let mut command = Command::new(executable);
    command.args(["--ignored", "--exact", FIXTURE_TEST, "--test-threads=1"])
        .env("ASB_RESOURCE_FIXTURE_DIRECTORY", directory).env("ASB_RESOURCE_FIXTURE_BRANCH", branch)
        .env("ASB_RESOURCE_FIXTURE_ROLE", role).stdin(Stdio::null())
        .stdout(fs::File::create(directory.join(format!("{branch}-{role}.stdout"))).unwrap())
        .stderr(fs::File::create(directory.join(format!("{branch}-{role}.stderr"))).unwrap());
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

fn executable(source: &Path, destination: &Path) -> PathBuf {
    let destination = destination.with_extension(if cfg!(windows) { "exe" } else { "" });
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::hard_link(source, &destination).unwrap();
    destination
}

fn wait_pid(directory: &Path, branch: &str, role: &str) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(30);
    let path = directory.join(format!("{branch}-{role}.pid"));
    loop {
        if let Ok(value) = fs::read_to_string(&path) {
            if let Ok(pid) = value.parse() { return pid; }
        }
        assert!(Instant::now() < deadline, "fixture did not become ready: {}", path.display());
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn native_editor_engines_and_their_real_task_processes_have_one_owner() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let payload = root.join("fixture-payload");
    fs::copy(std::env::current_exe().unwrap(), &payload).unwrap();
    let mut children = Children { stop: root.join("stop"), processes: Vec::new() };
    let mut expected = Vec::new();
    for (branch, group, entry, package, helper) in [
        ("codex", Group::Codex, "bin/windows-x86_64/codex", r#"{"name":"chatgpt","publisher":"openai"}"#, "codex-code-mode-host"),
        ("claude", Group::Claude, "resources/native-binary/claude", r#"{"name":"claude-code","publisher":"Anthropic"}"#, "claude-helper"),
    ] {
        let package_root = root.join(branch);
        fs::create_dir_all(&package_root).unwrap();
        fs::write(package_root.join("package.json"), package).unwrap();
        let engine = executable(&payload, &package_root.join(entry));
        let helper = executable(&payload, &root.join("helpers").join(helper));
        let task = executable(&payload, &root.join("tasks").join(format!("{branch}-build-task")));
        let mut engine_command = command(&engine, root, branch, "engine");
        engine_command.env("ASB_RESOURCE_FIXTURE_HELPER", helper).env("ASB_RESOURCE_FIXTURE_TASK", task);
        children.processes.push(engine_command.spawn().unwrap());
        for role in ["engine", "helper", "task"] { expected.push((wait_pid(root, branch, role), group)); }
    }
    let service = executable(&payload, &root.join("helpers/codex-windows-sandbox-service"));
    children.processes.push(command(&service, root, "unrelated", "task").spawn().unwrap());
    expected.push((wait_pid(root, "unrelated", "task"), Group::Switchboard));
    let collection = discovery::collect(InterfaceProcesses { processes: Vec::new(),
        coverage: InterfaceCoverage::Partial, issues: Vec::new() }, &HashMap::new()).unwrap();
    for (pid, group) in expected {
        let matches: Vec<_> = collection.processes.iter().filter(|process| process.observation.key.pid == pid).collect();
        assert_eq!(matches.len(), 1, "expected one owner for fixture pid {pid}");
        assert_eq!(matches[0].owner.group, group, "fixture pid {pid}");
        assert!(matches[0].observation.key.birth > 0);
        assert!(matches[0].observation.resident_bytes.unwrap() > 0);
        assert!(matches[0].observation.cpu_ns.is_ok());
    }
}

#[test]
#[ignore = "native process fixture; launched only by the process-tree test"]
fn fixture_process() {
    let root = PathBuf::from(std::env::var_os("ASB_RESOURCE_FIXTURE_DIRECTORY").expect("isolated fixture directory"));
    let branch = std::env::var("ASB_RESOURCE_FIXTURE_BRANCH").unwrap();
    let role = std::env::var("ASB_RESOURCE_FIXTURE_ROLE").unwrap();
    let mut child = match role.as_str() {
        "engine" | "helper" => {
            let (variable, next) = if role == "engine" { ("ASB_RESOURCE_FIXTURE_HELPER", "helper") }
                else { ("ASB_RESOURCE_FIXTURE_TASK", "task") };
            let executable = PathBuf::from(std::env::var_os(variable).unwrap());
            Some(command(&executable, &root, &branch, next).spawn().unwrap())
        }
        "task" => None,
        _ => panic!("unknown fixture role"),
    };
    fs::write(root.join(format!("{branch}-{role}.pid")), std::process::id().to_string()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(40);
    while !root.join("stop").exists() && Instant::now() < deadline { thread::sleep(Duration::from_millis(10)); }
    let _ = fs::write(root.join("stop"), b"stop");
    if let Some(child) = child.as_mut() { child.wait().unwrap(); }
}
