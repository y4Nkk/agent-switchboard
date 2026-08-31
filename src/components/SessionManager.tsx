import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getSessionMessages,
  listSessions,
  resumeSession,
  type AppKind,
  type SessionIssue,
  type SessionMessage,
  type SessionMeta,
} from "../api/client";
import { ClientLogo } from "./ClientLogo";
import { Input } from "./Input";
import { Time } from "./Time";

type Filter = "all" | AppKind;

function clientName(app: AppKind): string {
  return app === "codex" ? "Codex" : "Claude Code";
}

function searchMatches(session: SessionMeta, query: string): boolean {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return true;
  return [session.sessionId, session.title, session.summary, session.projectDir]
    .filter((value): value is string => value !== null)
    .some((value) => value.toLocaleLowerCase().includes(needle));
}

function messageRole(role: string): string {
  switch (role.toLowerCase()) {
    case "user":
      return "用户";
    case "assistant":
      return "助手";
    case "system":
      return "系统";
    case "tool":
      return "工具";
    default:
      return role;
  }
}

async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

/**
 * Read-only local history browser. It mirrors the useful CC Switch flow
 * (scan → filter → detail → copy resume command) without importing its UI,
 * multi-client scope, terminal launcher, cache, or file mutation paths.
 */
export function SessionManager({ active }: { active: boolean }) {
  const [sessions, setSessions] = useState<SessionMeta[] | null>(null);
  const [issues, setIssues] = useState<SessionIssue[]>([]);
  const [scanError, setScanError] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<SessionMeta | null>(null);
  const [messages, setMessages] = useState<SessionMessage[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [messageLoading, setMessageLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<string | null>(null);
  const [resumeStatus, setResumeStatus] = useState<string | null>(null);
  const [resuming, setResuming] = useState(false);
  const requestVersion = useRef(0);
  const scanVersion = useRef(0);
  const scanRequest = useRef<Promise<Awaited<ReturnType<typeof listSessions>>> | null>(null);

  const refresh = useCallback(async () => {
    const version = ++scanVersion.current;
    setLoading(true);
    setCopyStatus(null);
    setScanError(null);
    try {
      const scan = await (scanRequest.current ?? (() => {
        const next = listSessions();
        scanRequest.current = next;
        void next.finally(() => {
          if (scanRequest.current === next) scanRequest.current = null;
        }).catch(() => {});
        return next;
      })());
      if (scanVersion.current !== version) return;
      setSessions(scan.sessions);
      setIssues(scan.issues);
      setSelected((current) =>
        current
          ? scan.sessions.find((session) => session.app === current.app && session.sessionId === current.sessionId) ?? null
          : null,
      );
    } catch (caught) {
      if (scanVersion.current !== version) return;
      setSessions([]);
      setIssues([]);
      setSelected(null);
      setScanError((caught as { message?: string }).message ?? "无法扫描本地会话");
    } finally {
      if (scanVersion.current === version) setLoading(false);
    }
  }, []);

  /* The instance stays mounted across page switches; scanning happens on
     activation only, and an in-flight scan is shared by re-activation. */
  useEffect(() => {
    if (active) void refresh();
  }, [active, refresh]);

  const filtered = useMemo(
    () =>
      (sessions ?? []).filter(
        (session) => (filter === "all" || session.app === filter) && searchMatches(session, query),
      ),
    [filter, query, sessions],
  );

  const selectSession = async (session: SessionMeta) => {
    setSelected(session);
    setMessages(null);
    setDetailError(null);
    setCopyStatus(null);
    setResumeStatus(null);
    const version = requestVersion.current + 1;
    requestVersion.current = version;
    setMessageLoading(true);
    try {
      const nextMessages = await getSessionMessages(session.app, session.sessionId);
      if (requestVersion.current === version) setMessages(nextMessages);
    } catch (caught) {
      if (requestVersion.current === version) {
        setDetailError((caught as { message?: string }).message ?? "无法读取会话内容");
      }
    } finally {
      if (requestVersion.current === version) setMessageLoading(false);
    }
  };

  const copy = async (text: string, label: string) => {
    try {
      await copyText(text);
      setCopyStatus(`已复制${label}`);
    } catch {
      setCopyStatus(`无法复制${label}`);
    }
  };

  const resume = async () => {
    if (!selected || resuming) return;
    setResuming(true);
    setResumeStatus(null);
    try {
      const result = await resumeSession(selected.app, selected.sessionId);
      setResumeStatus(
        result.usedProjectDir
          ? "已在新命令提示符窗口中恢复会话"
          : "已在新命令提示符窗口中启动恢复；原工作目录不可用",
      );
    } catch (caught) {
      setResumeStatus((caught as { message?: string }).message ?? "无法启动会话恢复");
    } finally {
      setResuming(false);
    }
  };

  return (
    <div className="asb-sessions">
      <div className="asb-session-toolbar">
        <Input
          aria-label="搜索会话"
          value={query}
          placeholder="搜索标题、摘要、目录或会话 ID"
          onChange={(event) => setQuery(event.target.value)}
        />
        <div className="asb-segments" role="radiogroup" aria-label="会话客户端筛选">
          {(["all", "codex", "claude"] as const).map((item) => {
            const active = filter === item;
            const label = item === "all" ? "全部" : clientName(item);
            return (
              <label className={`asb-seg-opt${active ? " is-active" : ""}`} key={item}>
                <input
                  type="radio"
                  name="session-provider"
                  checked={active}
                  onChange={() => setFilter(item)}
                />
                {label}
              </label>
            );
          })}
        </div>
        <button type="button" className="asb-btn-secondary" disabled={loading} onClick={() => void refresh()}>
          刷新会话
        </button>
      </div>
      {issues.length > 0 && (
        <ul className="asb-session-issues" aria-label="会话扫描提示">
          {issues.map((issue) => (
            <li key={`${issue.app}-${issue.message}`} className="asb-warn-text">
              {clientName(issue.app)}：{issue.message}
            </li>
          ))}
        </ul>
      )}
      {scanError && <p className="asb-warn-text" role="alert">{scanError}</p>}
      <div className="asb-session-layout">
        <section className="asb-session-list" aria-label="会话列表">
          <div className="asb-session-list-heading">
            <span>会话</span>
            <span className="asb-session-count">{filtered.length}</span>
          </div>
          {sessions === null || loading ? (
            <p className="asb-empty">正在扫描本地会话</p>
          ) : filtered.length === 0 ? (
            <p className="asb-empty">未找到匹配的 Codex 或 Claude Code 会话</p>
          ) : (
            <div className="asb-session-items">
              {filtered.map((session) => {
                const active = selected?.app === session.app && selected.sessionId === session.sessionId;
                return (
                  <button
                    type="button"
                    className={`asb-session-item${active ? " is-active" : ""}`}
                    key={`${session.app}-${session.sessionId}`}
                    aria-pressed={active}
                    onClick={() => void selectSession(session)}
                  >
                    <span className="asb-session-item-title">
                      <ClientLogo app={session.app} className="asb-session-logo" />
                      <span>{session.title}</span>
                    </span>
                    <span className="asb-session-item-summary">{session.summary}</span>
                    <span className="asb-session-item-time">
                      {session.lastActiveAt ? <Time iso={session.lastActiveAt} /> : "时间未知"}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </section>
        <section className="asb-session-detail" aria-label="会话详情">
          {!selected ? (
            <p className="asb-empty">选择一条会话即可查看内容并复制恢复命令。</p>
          ) : (
            <>
              <header className="asb-session-detail-head">
                <div>
                  <span className="asb-session-client">
                    <ClientLogo app={selected.app} className="asb-session-logo" />
                    {clientName(selected.app)}
                  </span>
                  <h3>{selected.title}</h3>
                </div>
                <div className="asb-session-actions">
                  <button type="button" className="asb-btn-primary" disabled={resuming} onClick={() => void resume()}>
                    {resuming ? "正在启动" : "在命令提示符中恢复"}
                  </button>
                  <button
                    type="button"
                    className="asb-btn-secondary"
                    onClick={() => void copy(selected.resumeCommand, "恢复命令")}
                  >
                    复制恢复命令
                  </button>
                  <button
                    type="button"
                    className="asb-btn-secondary"
                    disabled={!selected.projectDir}
                    onClick={() => selected.projectDir && void copy(selected.projectDir, "工作目录")}
                  >
                    复制工作目录
                  </button>
                </div>
              </header>
              <dl className="asb-session-meta">
                <div>
                  <dt>会话 ID</dt>
                  <dd className="asb-code">{selected.sessionId}</dd>
                </div>
                <div>
                  <dt>最近活跃</dt>
                  <dd>{selected.lastActiveAt ? <Time iso={selected.lastActiveAt} /> : "未知"}</dd>
                </div>
                <div>
                  <dt>工作目录</dt>
                  <dd className="asb-code">{selected.projectDir ?? "未记录"}</dd>
                </div>
                <div>
                  <dt>恢复命令</dt>
                  <dd className="asb-code">{selected.resumeCommand}</dd>
                </div>
              </dl>
              {resumeStatus && <p className="asb-scope-note" role="status">{resumeStatus}</p>}
              {copyStatus && <p className="asb-scope-note" role="status">{copyStatus}</p>}
              <div className="asb-session-transcript" aria-label="对话历史">
                {messageLoading && <p className="asb-empty">正在读取会话内容</p>}
                {detailError && <p className="asb-warn-text">{detailError}</p>}
                {messages !== null && messages.length === 0 && <p className="asb-empty">会话中没有可展示的消息。</p>}
                {messages?.map((message, index) => (
                  <article className={`asb-session-message is-${message.role.toLowerCase()}`} key={`${message.at ?? ""}-${index}`}>
                    <header>
                      <span>{messageRole(message.role)}</span>
                      {message.at && <Time iso={message.at} />}
                    </header>
                    <pre>{message.content}</pre>
                  </article>
                ))}
              </div>
            </>
          )}
        </section>
      </div>
    </div>
  );
}
