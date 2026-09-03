//! Native Windows system-tray integration.
//!
//! The menu exposes the same two-client provider selection as the main
//! surface. A tray click first creates the established switch preview, then
//! passes its hashes to the existing transaction executor; this module never
//! renders or writes a client configuration by itself.

use crate::commands::{config_status_report, ConfigFileStatus};
use crate::local_state::{AppSettings, CloseBehavior, LocalState};
use asb_core::contracts::{
    AppKind, MatchStatus, ProviderProfile, ProviderRecord, UsageReading, UsageSummary,
};
use chrono::{DateTime, Local};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};

const TRAY_ID: &str = "agent-switchboard-tray";
const MENU_SHOW: &str = "tray-show-window";
const MENU_QUIT: &str = "tray-quit";
const MENU_SWITCH_PREFIX: &str = "tray-switch:";
const MENU_CODEX: &str = "tray-codex";
const MENU_CLAUDE: &str = "tray-claude";

static TRAY_READY: AtomicBool = AtomicBool::new(false);
static EXPLICIT_EXIT: AtomicBool = AtomicBool::new(false);
static SWITCHING: AtomicBool = AtomicBool::new(false);

pub fn is_ready() -> bool {
    TRAY_READY.load(Ordering::Relaxed)
}

pub(crate) fn keep_alive(tray_ready: bool, stored: Result<AppSettings, String>) -> bool {
    if !tray_ready {
        return false;
    }
    stored
        .map(|settings| settings.close_behavior == CloseBehavior::HideToTray)
        .unwrap_or(true)
}

pub fn request_explicit_exit() {
    EXPLICIT_EXIT.store(true, Ordering::Relaxed);
}

pub fn take_explicit_exit() -> bool {
    EXPLICIT_EXIT.swap(false, Ordering::Relaxed)
}

pub(crate) fn should_absorb(app: &AppHandle<Wry>) -> bool {
    let stored = LocalState::from_app(app).and_then(|state| state.get_app_settings());
    keep_alive(is_ready(), stored)
}

fn client_name(app: AppKind) -> &'static str {
    match app {
        AppKind::Codex => "Codex",
        AppKind::Claude => "Claude",
    }
}

fn submenu_id(app: AppKind) -> &'static str {
    match app {
        AppKind::Codex => MENU_CODEX,
        AppKind::Claude => MENU_CLAUDE,
    }
}

fn switch_menu_id(profile_id: &str) -> String {
    format!("{MENU_SWITCH_PREFIX}{profile_id}")
}

fn profile_id_from_menu(event_id: &str) -> Option<&str> {
    let profile_id = event_id.strip_prefix(MENU_SWITCH_PREFIX)?;
    (!profile_id.is_empty()).then_some(profile_id)
}

fn number(value: f64) -> String {
    let rounded = format!("{value:.2}");
    rounded
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn number_with_unit(value: f64, unit: Option<&str>) -> String {
    let value = number(value);
    match unit.map(str::trim).filter(|unit| !unit.is_empty()) {
        Some("%") => format!("{value}%"),
        Some(unit) => format!("{value} {unit}"),
        None => value,
    }
}

fn usage_reading_label(reading: &UsageReading) -> Option<String> {
    let plan = reading
        .plan_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("用量");
    let unit = reading.unit.as_deref();
    let mut values = Vec::new();
    if let Some(remaining) = reading.remaining {
        values.push(format!("余 {}", number_with_unit(remaining, unit)));
    }
    if let Some(used) = reading.used {
        values.push(format!("已用 {}", number_with_unit(used, unit)));
    }
    if let Some(total) = reading.total {
        values.push(format!("总 {}", number_with_unit(total, unit)));
    }
    (!values.is_empty()).then(|| format!("{plan} · {}", values.join(" · ")))
}

fn compact_usage_reading_label(reading: &UsageReading) -> Option<String> {
    let plan = reading
        .plan_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("用量");
    let unit = reading.unit.as_deref();
    if let Some(remaining) = reading.remaining {
        return Some(format!("{plan} 余 {}", number_with_unit(remaining, unit)));
    }
    if let Some(used) = reading.used {
        return Some(format!("{plan} 已用 {}", number_with_unit(used, unit)));
    }
    reading
        .total
        .map(|total| format!("{plan} 总 {}", number_with_unit(total, unit)))
}

fn usage_updated_label(summary: &UsageSummary) -> String {
    DateTime::parse_from_rfc3339(&summary.at)
        .map(|timestamp| {
            format!(
                "更新于 {}",
                timestamp.with_timezone(&Local).format("%m-%d %H:%M")
            )
        })
        .unwrap_or_else(|_| "最近成功结果".to_string())
}

/// Returns every profile whose last successful query still belongs to its
/// current query definition. The tray treats this as a client-wide section;
/// the active profile only controls its selection checkmark.
fn cached_usage_profiles<'a>(
    state: &LocalState,
    records: &'a [ProviderRecord],
) -> Vec<(&'a ProviderProfile, UsageSummary)> {
    records
        .iter()
        .filter_map(|record| {
            crate::usage_cache::get(state, &record.profile)
                .map(|summary| (&record.profile, summary))
        })
        .collect()
}

fn active_profile_id(statuses: &[ConfigFileStatus], app: AppKind) -> Option<&str> {
    statuses
        .iter()
        .find(|status| status.app == app)
        .and_then(|status| match &status.match_status {
            MatchStatus::MatchesProfile { profile_id, .. } => Some(profile_id.as_str()),
            _ => None,
        })
}

fn fallback_title(statuses: &[ConfigFileStatus], app: AppKind) -> &'static str {
    let Some(status) = statuses.iter().find(|status| status.app == app) else {
        return "未加载";
    };
    if status.read_error.is_some() {
        "配置不可读"
    } else if !status.exists {
        "未加载"
    } else {
        "未由供应商管理"
    }
}

fn menu_title(
    statuses: &[ConfigFileStatus],
    app: AppKind,
    active: Option<&ProviderProfile>,
    cached_usage: &[(&ProviderProfile, UsageSummary)],
) -> String {
    let client = client_name(app);
    let balances = cached_usage
        .iter()
        .filter_map(|(profile, summary)| {
            summary
                .readings
                .iter()
                .find_map(compact_usage_reading_label)
                .map(|reading| format!("{} · {reading}", profile.name))
        })
        .collect::<Vec<_>>();
    if !balances.is_empty() {
        return format!("{client} · {}", balances.join(" · "));
    }
    match active {
        Some(profile) => format!("{client} · {}", profile.name),
        None => format!("{client} · {}", fallback_title(statuses, app)),
    }
}

fn app_submenu(
    app: &AppHandle<Wry>,
    kind: AppKind,
    records: &[ProviderRecord],
    statuses: &[ConfigFileStatus],
    state: &LocalState,
) -> tauri::Result<Submenu<Wry>> {
    let active_id = active_profile_id(statuses, kind);
    let active = active_id.and_then(|id| {
        records
            .iter()
            .find(|record| record.profile.id == id)
            .map(|record| &record.profile)
    });
    let cached_usage = cached_usage_profiles(state, records);
    let submenu = Submenu::with_id(
        app,
        submenu_id(kind),
        menu_title(statuses, kind, active, &cached_usage),
        true,
    )?;
    if records.is_empty() {
        let empty = MenuItem::with_id(
            app,
            format!("tray-{}-empty", submenu_id(kind)),
            "尚无供应商",
            false,
            None::<&str>,
        )?;
        submenu.append(&empty)?;
        return Ok(submenu);
    }

    for record in records {
        let active = active_id == Some(record.profile.id.as_str());
        let item = CheckMenuItem::with_id(
            app,
            switch_menu_id(&record.profile.id),
            record.profile.name.clone(),
            !active,
            active,
            None::<&str>,
        )?;
        submenu.append(&item)?;
    }

    if !cached_usage.is_empty() {
        let separator = PredefinedMenuItem::separator(app)?;
        let heading = MenuItem::with_id(
            app,
            format!("tray-usage-heading:{}", submenu_id(kind)),
            "最近查询的用量",
            false,
            None::<&str>,
        )?;
        submenu.append(&separator)?;
        submenu.append(&heading)?;
        for (profile, summary) in cached_usage {
            let provider = MenuItem::with_id(
                app,
                format!("tray-usage-provider:{}", profile.id),
                format!("{} · {}", profile.name, usage_updated_label(&summary)),
                false,
                None::<&str>,
            )?;
            submenu.append(&provider)?;
            for (index, reading) in summary.readings.iter().enumerate() {
                let Some(label) = usage_reading_label(reading) else {
                    continue;
                };
                let item = MenuItem::with_id(
                    app,
                    format!("tray-usage-reading:{}:{index}", profile.id),
                    label,
                    false,
                    None::<&str>,
                )?;
                submenu.append(&item)?;
            }
        }
    }
    Ok(submenu)
}

fn build_menu(app: &AppHandle<Wry>) -> tauri::Result<Menu<Wry>> {
    let state = LocalState::from_app(app).map_err(tauri::Error::AssetNotFound)?;
    let records = state
        .configuration()
        .list_providers()
        .map_err(|error| tauri::Error::AssetNotFound(error.to_string()))?;
    let statuses =
        config_status_report(&state).map_err(|error| tauri::Error::AssetNotFound(error.message))?;
    let show = MenuItem::with_id(app, MENU_SHOW, "打开主界面", true, None::<&str>)?;
    let claude_records = records
        .iter()
        .filter(|record| record.profile.app == AppKind::Claude)
        .cloned()
        .collect::<Vec<_>>();
    let codex_records = records
        .iter()
        .filter(|record| record.profile.app == AppKind::Codex)
        .cloned()
        .collect::<Vec<_>>();
    let claude = app_submenu(app, AppKind::Claude, &claude_records, &statuses, &state)?;
    let codex = app_submenu(app, AppKind::Codex, &codex_records, &statuses, &state)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &show,
            &separator_one,
            &claude,
            &codex,
            &separator_two,
            &quit,
        ],
    )
}

/// Keeps the native recovery route available even if app-owned data is
/// currently unreadable. A later refresh replaces it with the live menu.
fn recovery_menu(app: &AppHandle<Wry>) -> tauri::Result<Menu<Wry>> {
    let show = MenuItem::with_id(app, MENU_SHOW, "打开主界面", true, None::<&str>)?;
    let unavailable = MenuItem::with_id(
        app,
        "tray-provider-data-unavailable",
        "供应商数据不可读",
        false,
        None::<&str>,
    )?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[&show, &separator_one, &unavailable, &separator_two, &quit],
    )
}

fn show_main_window(app: &AppHandle<Wry>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        // Close-to-tray can hide the window while it is minimized; SW_SHOW
        // keeps that state, so the restore must be explicit. A zoomed window
        // must not be restored here, or it would lose its maximized size.
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        let _ = window.set_focus();
    }
}

async fn switch_from_tray(app: AppHandle<Wry>, profile_id: String) {
    let result = async {
        let preview =
            crate::commands::switching::preview_switch(app.clone(), profile_id.clone()).await?;
        crate::commands::switching::execute_switch(
            app.clone(),
            profile_id,
            preview.content_hash,
            preview.rendered_hash,
            true,
        )
        .await
    }
    .await;
    if let Err(error) = result {
        log::warn!("托盘切换供应商失败: {}", error.message);
    }
    SWITCHING.store(false, Ordering::Release);
    refresh(&app);
}

fn request_switch(app: &AppHandle<Wry>, profile_id: &str) {
    if SWITCHING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    tauri::async_runtime::spawn(switch_from_tray(app.clone(), profile_id.to_string()));
}

/// Rebuilds the dynamic menu from current profiles, real-client status, and
/// cached usage. No provider query runs here.
pub fn refresh(app: &AppHandle<Wry>) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let Ok(menu) = build_menu(app).or_else(|_| recovery_menu(app)) else {
        return;
    };
    let _ = tray.set_menu(Some(menu));
}

/// Creates the always-visible tray icon. Left click restores the main window;
/// right click opens Codex and Claude provider submenus. Selecting a provider
/// is an explicit user write confirmation routed through the shared executor.
pub fn setup(app: &AppHandle<Wry>) -> tauri::Result<()> {
    let menu = build_menu(app).or_else(|_| recovery_menu(app))?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("默认应用图标".to_string()))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Agent Switchboard")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_SHOW => show_main_window(app),
            MENU_QUIT => {
                request_explicit_exit();
                app.exit(0);
            }
            event_id => {
                if let Some(profile_id) = profile_id_from_menu(event_id) {
                    request_switch(app, profile_id);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    TRAY_READY.store(true, Ordering::Relaxed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_state::{MotionPreference, ThemePreference};
    use crate::runtime_log::RuntimeLogLevel;
    use asb_core::contracts::{ProviderDraft, RouteMode, UsageQuery, UsageReading};

    fn profile(id: &str, name: &str) -> ProviderProfile {
        ProviderProfile::from_draft(
            id.to_string(),
            ProviderDraft {
                app: AppKind::Claude,
                route_mode: RouteMode::Custom,
                name: name.to_string(),
                model: None,
                base_url: Some("https://example.invalid".to_string()),
                api_key: "test-key".to_string(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: Some(UsageQuery::Declarative {
                    url: "{{baseUrl}}/usage".to_string(),
                    remaining_path: Some("/remaining".to_string()),
                    used_path: None,
                    total_path: None,
                    unit: Some("%".to_string()),
                    refresh_interval_minutes: 0,
                }),
            },
        )
    }

    fn summary(at: &str) -> UsageSummary {
        UsageSummary {
            readings: vec![UsageReading {
                plan_name: Some("额度".to_string()),
                remaining: Some(92.0),
                used: Some(8.0),
                total: Some(100.0),
                unit: Some("%".to_string()),
            }],
            at: at.to_string(),
        }
    }

    #[test]
    fn settings_error_keeps_the_app_recoverable_when_tray_exists() {
        assert!(keep_alive(true, Err("应用设置不可读".to_string())));
    }

    #[test]
    fn exit_setting_allows_close_even_when_tray_exists() {
        assert!(!keep_alive(
            true,
            Ok(AppSettings {
                close_behavior: CloseBehavior::Exit,
                theme: ThemePreference::System,
                motion: MotionPreference::System,
                always_on_top: false,
                launch_at_login: false,
                hardware_acceleration: true,
                interface_font: "Noto Sans SC".to_string(),
                runtime_log_level: RuntimeLogLevel::Info,
                collapsed_usage_ids: Vec::new(),
            })
        ));
    }

    #[test]
    fn hide_requires_a_real_tray_icon() {
        assert!(!keep_alive(false, Ok(AppSettings::default())));
    }

    #[test]
    fn explicit_exit_marker_is_consumed_once() {
        EXPLICIT_EXIT.store(false, Ordering::Relaxed);
        request_explicit_exit();
        assert!(take_explicit_exit());
        assert!(!take_explicit_exit());
    }

    #[test]
    fn tray_menu_ids_accept_only_nonempty_provider_ids() {
        assert_eq!(
            profile_id_from_menu("tray-switch:provider-a"),
            Some("provider-a")
        );
        assert_eq!(profile_id_from_menu("tray-switch:"), None);
        assert_eq!(profile_id_from_menu("other:provider-a"), None);
    }

    #[test]
    fn usage_reading_labels_keep_each_quota_on_one_clear_line() {
        let reading = UsageReading {
            plan_name: Some("MAX · 5 小时额度".to_string()),
            remaining: Some(92.0),
            used: Some(8.0),
            total: Some(100.0),
            unit: Some("%".to_string()),
        };
        assert_eq!(
            usage_reading_label(&reading),
            Some("MAX · 5 小时额度 · 余 92% · 已用 8% · 总 100%".to_string())
        );
    }

    #[test]
    fn cached_usage_section_includes_every_supplier_with_a_current_result() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let first = profile("provider-a", "供应商 A");
        let second = profile("provider-b", "供应商 B");
        let not_queried = profile("provider-c", "供应商 C");
        crate::usage_cache::store(&state, &first, summary("2026-09-02T02:34:00Z"))
            .expect("store first result");
        crate::usage_cache::store(&state, &second, summary("2026-09-02T02:35:00Z"))
            .expect("store second result");
        let records = vec![
            ProviderRecord {
                profile: first,
                file_hash: "first".to_string(),
            },
            ProviderRecord {
                profile: second,
                file_hash: "second".to_string(),
            },
            ProviderRecord {
                profile: not_queried,
                file_hash: "third".to_string(),
            },
        ];

        let cached_usage = cached_usage_profiles(&state, &records);
        let displayed = cached_usage
            .iter()
            .map(|(profile, _)| profile.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(displayed, ["供应商 A", "供应商 B"]);
        assert_eq!(
            menu_title(&[], AppKind::Claude, None, &cached_usage),
            "Claude · 供应商 A · 额度 余 92% · 供应商 B · 额度 余 92%"
        );
    }
}
