import type { ReactNode } from "react";
import type { CommandError } from "../api/client";
import { PinTopButton } from "../components/PinTopButton";
import { UpdateButton } from "../components/UpdateButton";
import { Button } from "../components/Button";
import { WindowControls } from "../components/WindowControls";
import appIcon from "../assets/app-icon.png";
import { isBrowserDevelopment } from "../lib/runtime";

export const PAGES = ["概览", "供应商", "通用设置", "用量", "会话", "日志", "备份", "发现", "设置"] as const;
export type Page = (typeof PAGES)[number];

interface AppShellProps {
  page: Page;
  onPageChange: (page: Page) => void;
  /** Persistent decision errors only; one-shot feedback lives in the global
   * toaster. */
  error: CommandError | null;
  busy: boolean;
  /** Why application settings could not load; the banner keeps the repair
   * reachable from every page while settings-backed actions silently wait. */
  settingsError: string | null;
  /** Replaces an unreadable settings file with validated defaults. */
  onRepairSettings: () => void;
  /** Offered only for an unsupported profile store. */
  onResetStore: () => void;
  /** Always-on-top toggle state; null until settings load. */
  pin: { active: boolean; onToggle: () => void } | null;
  /** Update indicator; present only while a newer release is known. */
  update: { latestVersion: string; onOpen: () => void } | null;
  children: ReactNode;
}

/** Application frame: brand, primary navigation, the persistent-error
 * banner, and the busy overlay. Page geometry never changes when banners
 * appear or disappear. */
export function AppShell({
  page,
  onPageChange,
  error,
  busy,
  settingsError,
  onRepairSettings,
  onResetStore,
  pin,
  update,
  children,
}: AppShellProps) {
  return (
    <>
      <div className="asb-ambient" aria-hidden="true" />
      <div className="asb-shell">
        <header className="asb-topbar asb-surface-rail" data-tauri-drag-region>
          <span className="asb-topbar-brand" data-tauri-drag-region>
            <img className="asb-topbar-icon" src={appIcon} alt="" />
            <h1 className="asb-topbar-title" data-tauri-drag-region>
              Agent Switchboard
            </h1>
          </span>
          <nav aria-label="主导航">
            <ul className="asb-nav">
              {PAGES.map((item) => (
                <li key={item}>
                  <button
                    type="button"
                    aria-label={item}
                    aria-current={page === item ? "page" : undefined}
                    onClick={() => onPageChange(item)}
                  >
                    {item}
                  </button>
                </li>
              ))}
            </ul>
          </nav>
          {isBrowserDevelopment ? <span className="asb-web-development-badge">浏览器开发 · 本机后端</span> : null}
          {update ? (
            <UpdateButton latestVersion={update.latestVersion} onOpen={update.onOpen} />
          ) : null}
          {pin ? (
            <PinTopButton active={pin.active} disabled={busy} onToggle={pin.onToggle} />
          ) : null}
          {!isBrowserDevelopment && <WindowControls />}
        </header>
        <div className="asb-workspace">
          <div className="asb-banner-stack" aria-label="操作状态">
            {/* Persistent decision errors only: the banner carries the store
                reset entry. One-shot operation feedback lives in the global
                toaster (DESIGN.md §7/§8). */}
            {error && (
              <div className="asb-banner asb-banner-error" role="alert" aria-label="操作错误">
                <span>{error.message}</span>
                {error.code === "profile-store-unsupported" && (
                  <Button
                    variant="danger"
                    disabled={busy}
                    onClick={onResetStore}
                  >
                    清空旧档案并重新开始
                  </Button>
                )}
              </div>
            )}
            {settingsError && (
              <div className="asb-banner asb-banner-error" role="alert" aria-label="应用设置不可用">
                <span>应用设置不可用：{settingsError}</span>
                <Button
                  variant="secondary"
                  disabled={busy}
                  onClick={onRepairSettings}
                >
                  一键修复
                </Button>
              </div>
            )}
          </div>
          {children}
        </div>
      </div>
      {busy && (
        <div className="asb-busy" role="status" aria-label="处理中">
          处理中
        </div>
      )}
    </>
  );
}
