import type { MessageEntry } from "../types";

/** Tray popup panel (tray.html entry). Language follows the tray snapshot's
 * app settings through the same provider as the main window. */
export const messages = {
  "tray.retry": ["重新读取", "Read again"],
  "tray.panel.aria": ["Agent Switchboard 托盘", "Agent Switchboard tray"],
  "tray.loading": ["正在读取供应商…", "Reading providers…"],
  "tray.emptyGroup": ["尚未添加供应商", "No providers added"],
  "tray.openMain": ["打开主界面", "Open main window"],
  "tray.quit": ["退出应用", "Quit app"],
  "tray.state.queryFailed": ["查询失败", "Query failed"],
  "tray.state.signInRequired": ["待登录", "Sign-in required"],
  "tray.state.reauthRequired": ["需重新登录", "Re-authentication required"],
  "tray.state.stale": ["上次读数", "Last reading"],
  "tray.usage.refresh": ["刷新用量", "Refresh usage"],
  "tray.usage.lastReading": ["上次读数：{time}", "Last reading: {time}"],
  "tray.actionFailed": ["托盘操作失败，请打开主界面检查。", "The tray action failed; open the main window to check."],
  "tray.provider.active": ["{name}，当前供应商", "{name}, current provider"],
  "tray.provider.switchTo": ["切换到 {name}", "Switch to {name}"],
  "tray.provider.confirmSwitch": ["再次点击切换到 {name}", "Click again to switch to {name}"],
  "tray.provider.switch": ["切换", "Switch"],
  "tray.provider.current": ["当前使用", "Current"],
  "tray.provider.switching": ["切换中", "Switching"],
} as const satisfies Record<string, MessageEntry>;
