//! Read-only discovery CLI. Used to verify that discovery changes nothing:
//!
//! ```powershell
//! # hash and timestamps before
//! Get-FileHash / Get-Item
//! cargo run -p agent-switchboard --bin asb-discover
//! # hash and timestamps after — must be identical
//! ```
//!
//! The report prints only existence, parse state, managed markers, warnings
//! and proposal names. It never prints file content or endpoint URLs.

use agent_switchboard_lib::local_config_paths;
use asb_core::discovery::{self, DiscoveredState};

fn main() {
    let paths = match local_config_paths() {
        Ok(paths) => paths,
        Err(message) => {
            eprintln!("无法定位本机配置：{message}");
            std::process::exit(1);
        }
    };
    let read = |p: &str| match std::fs::read_to_string(p) {
        Ok(text) if std::path::Path::new(p).is_file() => Ok(Some(text)),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("无法读取配置文件".to_owned()),
    };
    let report = discovery::discover(&paths, read);

    for file in [&report.codex, &report.claude] {
        let app = if file.app == asb_core::AppKind::Codex {
            "Codex"
        } else {
            "Claude Code"
        };
        match &file.state {
            DiscoveredState::Missing => println!("{app}：未找到（{}）", file.path),
            DiscoveredState::ReadError { message } => println!("{app}：读取失败：{message}"),
            DiscoveredState::ParseError { message, line } => {
                println!("{app}：格式错误（第 {line:?} 行）：{message}")
            }
            DiscoveredState::Ok {
                managed, warnings, ..
            } => {
                println!("{app}：有效，托管状态={managed}，警告数={}", warnings.len());
                for warning in warnings {
                    println!("  警告：{warning}");
                }
            }
        }
    }
    println!("可导入配置数：{}", report.import_proposals.len());
}
