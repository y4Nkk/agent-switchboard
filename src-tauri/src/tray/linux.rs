//! Linux tray activation via StatusNotifierItem. The panel is the same WebView
//! as other platforms; no D-Bus menu is exported.

use std::time::Duration;
use tauri::{AppHandle, Manager, PhysicalPosition};
use zbus::blocking::{connection::Builder, Connection, Proxy};

const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const ITEM_PATH: &str = "/StatusNotifierItem";

struct TrayConnection {
    _connection: Connection,
}

struct StatusNotifierItem {
    app: AppHandle,
    pixmap: Vec<(i32, i32, Vec<u8>)>,
}

impl StatusNotifierItem {
    fn toggle(&self, x: i32, y: i32) {
        let app = self.app.clone();
        if let Err(error) = self.app.run_on_main_thread(move || {
            if let Err(error) = super::popup::toggle(
                &app,
                None,
                Some(PhysicalPosition::new(f64::from(x), f64::from(y))),
            ) {
                super::recover_main(&app, &error);
            }
        }) {
            log::warn!("无法调度 Linux 托盘浮层: {error}");
        }
    }
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    fn activate(&self, x: i32, y: i32) {
        self.toggle(x, y);
    }

    fn context_menu(&self, x: i32, y: i32) {
        self.toggle(x, y);
    }

    fn secondary_activate(&self, x: i32, y: i32) {
        self.toggle(x, y);
    }

    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        "agent-switchboard"
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        "Agent Switchboard"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn window_id(&self) -> u32 {
        0
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        self.pixmap.clone()
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }
}

fn register(connection: &Connection, service: &str) -> Result<(), zbus::Error> {
    let watcher = Proxy::new(connection, WATCHER, WATCHER_PATH, WATCHER)?;
    watcher.call("RegisterStatusNotifierItem", &(service,))
}

fn register_and_report(connection: &Connection, service: &str) {
    match register(connection, service) {
        Ok(()) => super::set_ready(true),
        Err(error) => {
            super::set_ready(false);
            log::warn!("Linux 托盘注册失败: {error}");
        }
    }
}

fn argb(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|pixel| [pixel[3], pixel[0], pixel[1], pixel[2]])
        .collect()
}

pub fn setup(app: &AppHandle) -> Result<(), String> {
    let icon = app.default_window_icon().ok_or("默认应用图标不可用")?;
    let item = StatusNotifierItem {
        app: app.clone(),
        pixmap: vec![(icon.width() as i32, icon.height() as i32, argb(icon.rgba()))],
    };
    let service = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    let connection = Builder::session()
        .and_then(|builder| builder.name(service.clone()))
        .and_then(|builder| builder.serve_at(ITEM_PATH, item))
        .and_then(|builder| builder.method_timeout(Duration::from_secs(3)).build())
        .map_err(|error| format!("Linux 托盘 D-Bus 初始化失败: {error}"))?;
    let bus = Proxy::new(
        &connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .map_err(|error| format!("Linux 托盘 D-Bus 监听初始化失败: {error}"))?;
    // Subscribe before registering, so a panel restart between the two calls
    // cannot leave the icon permanently unregistered.
    let changes = bus
        .receive_signal_with_args("NameOwnerChanged", &[(0, WATCHER)])
        .map_err(|error| format!("Linux 托盘监听失败: {error}"))?;
    let available: bool = bus
        .call("NameHasOwner", &(WATCHER,))
        .map_err(|error| format!("Linux 托盘宿主查询失败: {error}"))?;
    if available {
        register_and_report(&connection, &service);
    } else {
        super::set_ready(false);
        log::info!("Linux 桌面尚未提供 StatusNotifierWatcher，等待托盘宿主");
    }
    let watch_connection = connection.clone();
    let watch_app = app.clone();
    std::thread::Builder::new()
        .name("tray-status-notifier".into())
        .spawn(move || {
            for message in changes {
                match message.body().deserialize::<(String, String, String)>() {
                    Ok((_, _, owner)) if owner.is_empty() => {
                        super::set_ready(false);
                        let app = watch_app.clone();
                        let _ = watch_app.run_on_main_thread(move || {
                            super::recover_main(&app, "桌面托盘宿主已退出")
                        });
                    }
                    Ok(_) => register_and_report(&watch_connection, &service),
                    Err(error) => log::warn!("Linux 托盘宿主事件解析失败: {error}"),
                }
            }
            super::set_ready(false);
            log::warn!("Linux 托盘 D-Bus 监听已断开");
            let app = watch_app.clone();
            let _ = watch_app
                .run_on_main_thread(move || super::recover_main(&app, "桌面托盘连接已断开"));
        })
        .map_err(|error| format!("Linux 托盘监听线程启动失败: {error}"))?;
    app.manage(TrayConnection {
        _connection: connection,
    });
    Ok(())
}

pub(super) fn is_wayland(window: &tauri::WebviewWindow) -> Result<bool, String> {
    use gtk::prelude::*;
    let window = window.gtk_window().map_err(|error| error.to_string())?;
    Ok(window.display().type_().name() == "GdkWaylandDisplay")
}

#[cfg(test)]
mod tests {
    use super::argb;

    #[test]
    fn status_notifier_pixmap_uses_network_argb_bytes() {
        assert_eq!(
            argb(&[10, 20, 30, 40, 50, 60, 70, 80]),
            [40, 10, 20, 30, 80, 50, 60, 70]
        );
    }
}
