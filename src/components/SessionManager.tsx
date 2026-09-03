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
import { Button } from "./Button";
import { Input } from "./Input";
import { Time } from "./Time";
import { toast } from "./use-toast";

type Filter = "all" | AppKind;

const MESSAGE_COLLAPSE_THRESHOLD = 3000;
const MESSAGE_COLLAPSED_LENGTH = 1500;
const TARGET_HIGHLIGHT_MS = 2000;
/* In-memory cache lifetime, ported from the CC Switch query pattern: within
   the TTL a re-activation or re-selection serves cache with zero requests;
   past it the cache is shown while a refresh runs in the background. */
const SESSION_CACHE_TTL_MS = 30_000;
/* Upper bound on retained transcripts so a long-running tray process cannot
   accumulate unbounded message memory; the oldest fetched entry is evicted. */
const MAX_CACHED_TRANSCRIPTS = 16;

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

function directoryName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function previewLine(content: string): string {
  return content.split(/\s+/).join(" ").trim();
}

/* Codex records non-conversational payloads as user-role messages: AGENTS.md
   dumps, environment context, and the IDE context wrapper VS Code injects.
   The outline lists real prompts only, so these markers are protocol facts to
   match byte-for-byte, not display copy. */
const CODEX_IDE_CONTEXT_PREFIX = "# Context from my IDE setup:";
const CODEX_REQUEST_MARKER = "my request for codex";

function codexRequestHeadingPayload(line: string): string | null {
  if (!line.startsWith("#")) return null;
  const heading = line.replace(/^#+\s*/, "");
  if (!heading.toLowerCase().startsWith(CODEX_REQUEST_MARKER)) return null;
  const suffix = heading.slice(CODEX_REQUEST_MARKER.length).trimStart();
  if (!suffix) return "";
  if (!/^[:：\-—]/.test(suffix)) return null;
  return suffix.replace(/^[:：\-—\s]+/, "").trim();
}

/* The real prompt inside an IDE wrapper is the LAST request heading: earlier
   matches can be headings living inside quoted file content. */
function codexPromptFromIdeContext(content: string): string | null {
  const lines = content.replace(/\r\n/g, "\n").split("\n");
  let prompt: string | null = null;
  for (const [index, line] of lines.entries()) {
    const inline = codexRequestHeadingPayload(line.trim());
    if (inline === null) continue;
    if (inline) {
      prompt = inline;
      continue;
    }
    const following = lines.slice(index + 1).join("\n").trim();
    prompt = following || null;
  }
  return prompt;
}

/* Outline preview for a Codex user message: null hides the entry from the
   outline (injected payload with no real prompt), otherwise the prompt text. */
function codexOutlinePreview(content: string): string | null {
  const trimmed = content.trim();
  if (
    trimmed.startsWith("# AGENTS.md instructions for ") ||
    trimmed.startsWith("<environment_context>")
  ) {
    return null;
  }
  if (trimmed.startsWith(CODEX_IDE_CONTEXT_PREFIX)) {
    return codexPromptFromIdeContext(trimmed);
  }
  return content;
}

async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

function SessionMessageView({
  message,
  index,
  targeted,
  expanded,
  onToggleExpanded,
}: {
  message: SessionMessage;
  index: number;
  targeted: boolean;
  expanded: boolean;
  onToggleExpanded: (index: number) => void;
}) {
  const isLong = message.content.length > MESSAGE_COLLAPSE_THRESHOLD;
  const collapsed = isLong && !expanded;
  const display =
    collapsed ? `${message.content.slice(0, MESSAGE_COLLAPSED_LENGTH)}…` : message.content;

  const copy = async () => {
    try {
      await copyText(message.content);
      toast({ kind: "success", title: "已复制消息内容" });
    } catch {
      toast({ kind: "error", title: "无法复制消息内容" });
    }
  };

  return (
    <article
      className={`asb-session-message is-${message.role.toLowerCase()}${targeted ? " is-target" : ""}`}
      data-index={index}
    >
      <header>
        <span>{messageRole(message.role)}</span>
        <span className="asb-session-message-time">
          {message.at ? <Time iso={message.at} /> : null}
        </span>
        <button type="button" className="asb-session-message-copy" onClick={() => void copy()}>
          复制
        </button>
      </header>
      <pre>{display}</pre>
      {isLong && (
        <button
          type="button"
          className="asb-session-message-toggle"
          aria-expanded={expanded}
          onClick={() => onToggleExpanded(index)}
        >
          {expanded
            ? "收起"
            : `展开完整内容（约 ${Math.round(message.content.length / 1000)}k 字符）`}
        </button>
      )}
    </article>
  );
}

/**
 * Read-only local history browser. It mirrors the useful CC Switch flow
 * (scan → filter → detail → message outline jump → copy resume command)
 * without importing its UI, multi-client scope, terminal launcher, deletion,
 * or file mutation paths. Scan and transcript results are cached in memory
 * for `SESSION_CACHE_TTL_MS`, so revisiting the page or a recently viewed
 * session displays instantly without rescanning.
 */
export function SessionManager({ active }: { active: boolean }) {
  const [sessions, setSessions] = useState<SessionMeta[] | null>(null);
  const [issues, setIssues] = useState<SessionIssue[]>([]);
  const [scanError, setScanError] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<SessionMeta | null>(null);
  const [messages, setMessages] = useState<SessionMessage[] | null>(null);
  const [scanning, setScanning] = useState(false);
  const [messageLoading, setMessageLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<string | null>(null);
  const [resumeStatus, setResumeStatus] = useState<string | null>(null);
  const [resuming, setResuming] = useState(false);
  const [expandedMessages, setExpandedMessages] = useState<Set<number>>(() => new Set());
  const [targetMessage, setTargetMessage] = useState<number | null>(null);
  const requestVersion = useRef(0);
  const scanVersion = useRef(0);
  const scanRequest = useRef<Promise<Awaited<ReturnType<typeof listSessions>>> | null>(null);
  const lastScanAt = useRef(0);
  const messageCache = useRef(new Map<string, { fetchedAt: number; messages: SessionMessage[] }>());
  const transcriptRef = useRef<HTMLDivElement | null>(null);
  const highlightTimer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(highlightTimer.current), []);

  const refresh = useCallback(async () => {
    const version = ++scanVersion.current;
    setScanning(true);
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
      lastScanAt.current = Date.now();
      setSessions(scan.sessions);
      setIssues(scan.issues);
      setSelected((current) =>
        current
          ? scan.sessions.find((session) => session.app === current.app && session.sessionId === current.sessionId) ?? null
          : null,
      );
    } catch (caught) {
      if (scanVersion.current !== version) return;
      /* A failed refresh keeps the last displayed list (stale-while-revalidate);
         only a first-ever scan failure falls back to the empty state. */
      if (lastScanAt.current === 0) {
        setSessions([]);
        setIssues([]);
        setSelected(null);
      }
      setScanError((caught as { message?: string }).message ?? "无法扫描本地会话");
    } finally {
      if (scanVersion.current === version) setScanning(false);
    }
  }, []);

  /* The instance stays mounted across page switches. Activation serves the
     cached list directly inside the TTL and only rescans (in the background)
     when no scan has ever succeeded or the cache has gone stale; the decision
     reads the scan timestamp, never the list state, so a failed first scan
     does not immediately retrigger itself. */
  useEffect(() => {
    if (active && (lastScanAt.current === 0 || Date.now() - lastScanAt.current > SESSION_CACHE_TTL_MS)) {
      void refresh();
    }
  }, [active, refresh]);

  const filtered = useMemo(
    () =>
      (sessions ?? []).filter(
        (session) => (filter === "all" || session.app === filter) && searchMatches(session, query),
      ),
    [filter, query, sessions],
  );

  const outlineItems = useMemo(() => {
    const previewOf = (content: string): string | null =>
      selected?.app === "codex" ? codexOutlinePreview(content) : content;
    return (
      messages
        ?.map((message, index) => ({ message, index }))
        .filter(({ message }) => message.role.toLowerCase() === "user")
        .map(({ message, index }) => {
          const raw = previewOf(message.content);
          return raw === null ? null : { index, preview: previewLine(raw) };
        })
        .filter((item): item is { index: number; preview: string } => item !== null) ?? []
    );
  }, [messages, selected?.app]);

  const selectSession = async (session: SessionMeta) => {
    setSelected(session);
    setDetailError(null);
    setCopyStatus(null);
    setResumeStatus(null);
    setExpandedMessages(new Set());
    setTargetMessage(null);
    window.clearTimeout(highlightTimer.current);
    const version = requestVersion.current + 1;
    requestVersion.current = version;

    /* Recently viewed transcripts come straight from cache; a stale entry is
       shown immediately and revalidated in the background. */
    const key = `${session.app}:${session.sessionId}`;
    const cached = messageCache.current.get(key);
    if (cached) {
      setMessages(cached.messages);
      if (Date.now() - cached.fetchedAt <= SESSION_CACHE_TTL_MS) return;
    } else {
      setMessages(null);
      setMessageLoading(true);
    }
    try {
      const nextMessages = await getSessionMessages(session.app, session.sessionId);
      if (requestVersion.current !== version) return;
      messageCache.current.set(key, { fetchedAt: Date.now(), messages: nextMessages });
      if (messageCache.current.size > MAX_CACHED_TRANSCRIPTS) {
        const oldest = [...messageCache.current.entries()].sort(
          ([, a], [, b]) => a.fetchedAt - b.fetchedAt,
        )[0];
        if (oldest) messageCache.current.delete(oldest[0]);
      }
      setMessages(nextMessages);
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

  const toggleExpanded = useCallback((index: number) => {
    setExpandedMessages((current) => {
      const next = new Set(current);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  }, []);

  const jumpToMessage = useCallback((index: number) => {
    const node = transcriptRef.current?.querySelector<HTMLElement>(`[data-index="${index}"]`);
    /* Instant, not smooth: on multi-thousand-line transcripts the animation
       gets cancelled by the highlight re-render and strands the scroll. */
    node?.scrollIntoView?.({ block: "center" });
    setTargetMessage(index);
    window.clearTimeout(highlightTimer.current);
    highlightTimer.current = window.setTimeout(() => setTargetMessage(null), TARGET_HIGHLIGHT_MS);
  }, []);

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
        <Button variant="secondary" disabled={scanning} onClick={() => void refresh()}>
          刷新会话
        </Button>
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
          {sessions === null ? (
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
                <div className="asb-session-detail-title">
                  <span className="asb-session-client">
                    <ClientLogo app={selected.app} className="asb-session-logo" />
                    {clientName(selected.app)}
                  </span>
                  <h3>{selected.title}</h3>
                </div>
                <div className="asb-session-actions">
                  <Button variant="primary" disabled={resuming} onClick={() => void resume()}>
                    {resuming ? "正在启动" : "在命令提示符中恢复"}
                  </Button>
                  <Button
                    variant="secondary"
                    onClick={() => void copy(selected.resumeCommand, "恢复命令")}
                  >
                    复制恢复命令
                  </Button>
                  <Button
                    variant="secondary"
                    disabled={!selected.projectDir}
                    onClick={() => selected.projectDir && void copy(selected.projectDir, "工作目录")}
                  >
                    复制工作目录
                  </Button>
                </div>
              </header>
              <p className="asb-session-meta-line">
                <span className="asb-code asb-session-meta-id">{selected.sessionId}</span>
                <span>{selected.lastActiveAt ? <Time iso={selected.lastActiveAt} /> : "时间未知"}</span>
                {selected.projectDir && (
                  <button
                    type="button"
                    className="asb-session-meta-dir"
                    title={`${selected.projectDir}（点击复制）`}
                    onClick={() => selected.projectDir && void copy(selected.projectDir, "工作目录")}
                  >
                    {directoryName(selected.projectDir)}
                  </button>
                )}
              </p>
              <div className="asb-session-command">
                <code className="asb-code">{selected.resumeCommand}</code>
              </div>
              {resumeStatus && <p className="asb-scope-note" role="status">{resumeStatus}</p>}
              {copyStatus && <p className="asb-scope-note" role="status">{copyStatus}</p>}
              <div className="asb-session-body">
                <div className="asb-session-conversation">
                  <div className="asb-session-list-heading">
                    <span>对话记录</span>
                    <span className="asb-session-count">{messages?.length ?? 0}</span>
                  </div>
                  <div className="asb-session-transcript" ref={transcriptRef} aria-label="对话历史">
                    {messageLoading && <p className="asb-empty">正在读取会话内容</p>}
                    {detailError && <p className="asb-warn-text">{detailError}</p>}
                    {messages !== null && messages.length === 0 && <p className="asb-empty">会话中没有可展示的消息。</p>}
                    {messages?.map((message, index) => (
                      <SessionMessageView
                        key={`${message.at ?? ""}-${index}`}
                        message={message}
                        index={index}
                        targeted={targetMessage === index}
                        expanded={expandedMessages.has(index)}
                        onToggleExpanded={toggleExpanded}
                      />
                    ))}
                  </div>
                </div>
                {outlineItems.length > 2 && (
                  <nav className="asb-session-toc" aria-label="消息目录">
                    <div className="asb-session-toc-heading">消息目录</div>
                    <div className="asb-session-toc-items">
                      {outlineItems.map((item, outlineIndex) => (
                        <button
                          type="button"
                          key={item.index}
                          onClick={() => jumpToMessage(item.index)}
                        >
                          <span className="asb-session-toc-index">{outlineIndex + 1}</span>
                          <span className="asb-session-toc-preview">{item.preview}</span>
                        </button>
                      ))}
                    </div>
                  </nav>
                )}
              </div>
            </>
          )}
        </section>
      </div>
    </div>
  );
}
