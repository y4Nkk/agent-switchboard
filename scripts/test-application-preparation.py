"""Headless application-data acceptance. All executed tests receive isolated homes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

RUNTIME_PATHS = []

REPOSITORY = Path(__file__).resolve().parent.parent
DRIVER = "application_preparation::tests::sandbox_process_driver"
BOUNDARIES = [
    "preferences-recorded", "preferences-backed-up", "preferences-prepared", "preferences-enabled",
    "configuration-building", "configuration-prepared", "configuration-retired", "configuration-enabled",
]


def environment(sandbox):
    env = dict(os.environ)
    for name in list(env):
        upper = name.upper()
        if upper.endswith(("API_KEY", "TOKEN", "SECRET", "PASSWORD")) or upper in {
            "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "ASB_WEB_DEVELOPMENT", "ASB_PREPARATION_FAULT",
        }:
            env.pop(name, None)
    for name, relative in {
        "USERPROFILE": "home", "HOME": "home", "APPDATA": "appdata/roaming",
        "LOCALAPPDATA": "appdata/local", "XDG_DATA_HOME": "appdata", "XDG_CONFIG_HOME": "config",
        "TEMP": "tmp", "TMP": "tmp", "TMPDIR": "tmp", "CODEX_HOME": "home/.codex",
        "CLAUDE_CONFIG_DIR": "home/.claude",
    }.items():
        path = sandbox / relative
        path.mkdir(parents=True, exist_ok=True)
        env[name] = str(path)
    env["CARGO_HOME"] = os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))
    env["RUSTUP_HOME"] = os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup"))
    env["ASB_PREPARATION_SANDBOX"] = str(sandbox)
    env["NO_PROXY"] = "localhost,127.0.0.1,::1"
    env["PATH"] = os.pathsep.join([*RUNTIME_PATHS, env.get("PATH", "")])
    (sandbox / ".asb-preparation-sandbox").write_text("isolated preparation acceptance", encoding="utf-8")
    return env


def execute(command, env, log, expected=0, timeout=180):
    result = subprocess.run(command, cwd=REPOSITORY, env=env, capture_output=True, text=True,
                            encoding="utf-8", errors="replace", timeout=timeout,
                            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
    log.write_text(result.stdout + result.stderr, encoding="utf-8")
    if result.returncode != expected:
        raise RuntimeError(f"{command[0]} exited {result.returncode}; expected {expected}.\n{(result.stdout + result.stderr)[-10000:]}")
    return result.stdout


def inventory(root):
    return {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in root.rglob("*") if path.is_file()}


def build(env, output):
    command = ["cargo", "test", "--offline", "--locked", "-p", "agent-switchboard", "--lib",
               "--no-run", "--message-format=json"]
    stdout = execute(command, env, output / "build.log", timeout=1200)
    for line in stdout.splitlines():
        try:
            artifact = json.loads(line)
        except ValueError:
            continue
        if artifact.get("reason") == "compiler-artifact" and artifact.get("executable") and artifact["profile"]["test"]:
            return artifact["executable"]
    raise RuntimeError("Cargo did not report the library test executable")


def crash_case(executable, output, boundary):
    case = output / boundary
    case.mkdir()
    env = environment(case)
    command = [executable, "--exact", DRIVER, "--ignored", "--test-threads=1"]
    env["ASB_PREPARATION_MODE"] = "seed"
    execute(command, env, case / "seed.log")
    env.pop("ASB_PREPARATION_MODE")
    originals = {name: (case / "state" / name).read_bytes()
                 for name in ["settings.json", "configuration/client-settings/codex.json"]}
    sentinels = [case / "home/.codex/config.toml", case / "home/.codex/auth.json",
                 case / "home/.claude/settings.json", case / "home/.claude/.credentials.json"]
    for path in sentinels:
        path.write_bytes(b"sandbox-only-client-file-must-remain-unchanged")
    env["ASB_PREPARATION_FAULT"] = boundary
    execute(command, env, case / "interruption.log", expected=73)
    env.pop("ASB_PREPARATION_FAULT")
    execute(command, env, case / "recovery.log")
    report = json.loads((case / "result.json").read_text(encoding="utf-8"))
    assert report["configurationReady"] and not report["issues"], report
    state = case / "state"
    preference_backups = list(state.glob("settings-backup-*.json"))
    configuration_backups = list(state.glob("repair-backup-*/client-settings/codex.json"))
    assert len(preference_backups) == len(configuration_backups) == 1
    assert preference_backups[0].read_bytes() == originals["settings.json"]
    assert configuration_backups[0].read_bytes() == originals["configuration/client-settings/codex.json"]
    assert not (state / "settings-recovery.json").exists()
    assert not (state / "configuration-repair.json").exists()
    before = inventory(state)
    execute(command, env, case / "idempotence.log")
    repeated = json.loads((case / "result.json").read_text(encoding="utf-8"))
    assert repeated["repairedFiles"] == [] and repeated["backupPaths"] == []
    assert inventory(state) == before
    assert all(path.read_bytes() == b"sandbox-only-client-file-must-remain-unchanged" for path in sentinels)
    return {"boundary": boundary, "forcedExit": 73, "recovered": True,
            "originalBackupsVerified": True, "repeatWithoutWrites": True, "clientFilesUnchanged": True}


def source_revision():
    digest = hashlib.sha256()
    paths = []
    for directory in ["src", "src-tauri/src", "crates"]:
        paths.extend(path for path in (REPOSITORY / directory).rglob("*") if path.is_file())
    paths.extend(REPOSITORY / name for name in ["Cargo.toml", "Cargo.lock", "src-tauri/Cargo.toml"])
    for source in sorted(paths):
        digest.update(str(source.relative_to(REPOSITORY)).encode("utf-8"))
        digest.update(hashlib.sha256(source.read_bytes()).digest())
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--executable")
    arguments = parser.parse_args()
    base = REPOSITORY / ".tmp/application-preparation-sandbox"
    base.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="acceptance-", dir=base)).resolve()
    env = environment(output / "unit")
    source_before = source_revision()
    executable = str(Path(arguments.executable or build(env, output)).resolve())
    library = subprocess.check_output(["rustc", "--print", "target-libdir"], cwd=REPOSITORY, env=env,
        text=True, creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0).strip()
    RUNTIME_PATHS.extend([str(Path(executable).parent), str(Path(executable).parent.parent), library])
    env = environment(output / "unit")
    evidence = {"sandbox": str(output), "executable": executable, "reusedExecutable": bool(arguments.executable), "suites": [], "crashRecovery": [],
                "browserLaunched": False, "realConfigurationUsed": False}
    for suite in ["application_preparation::", "config_store::repair::", "local_state::settings_",
                  "local_state::workspace::", "commands::error::tests::"]:
        stdout = execute([executable, suite, "--test-threads=1"], env,
                         output / (suite.replace(":", "_") + ".log"))
        match = re.search(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", stdout)
        assert match, stdout
        result = {"name": suite, "passed": int(match[1]), "failed": int(match[2]), "ignored": int(match[3])}
        evidence["suites"].append(result)
        print(json.dumps(result), flush=True)
    for boundary in BOUNDARIES:
        result = crash_case(executable, output, boundary)
        evidence["crashRecovery"].append(result)
        print(f"recovered: {boundary}", flush=True)
    assert source_revision() == source_before, "Sources changed during acceptance; rebuild and rerun"
    evidence["sourceSha256"] = source_before
    evidence["passed"] = True
    path = output / "evidence.json"
    path.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"Evidence: {path}", flush=True)


if __name__ == "__main__":
    if os.name == "nt":
        import ctypes
        ctypes.windll.kernel32.SetErrorMode(0x8003)
    main()
