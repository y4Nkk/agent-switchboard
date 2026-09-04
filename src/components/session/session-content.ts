import type { AppKind, SessionMeta } from "../../api/client";

const CODEX_IDE_CONTEXT_PREFIX = "# Context from my IDE setup:";
const CODEX_REQUEST_MARKER = "my request for codex";

export function clientName(app: AppKind): string {
  return app === "codex" ? "Codex" : "Claude Code";
}

export function sessionMatchesSearch(session: SessionMeta, query: string): boolean {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return true;
  return [session.sessionId, session.title, session.summary, session.projectDir]
    .filter((value): value is string => value !== null)
    .some((value) => value.toLocaleLowerCase().includes(needle));
}

export function messageRole(role: string): string {
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

export function directoryName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function previewLine(content: string): string {
  return content.split(/\s+/).join(" ").trim();
}

/* Codex records non-conversational payloads as user-role messages: AGENTS.md
   dumps, environment context, and the IDE context wrapper VS Code injects.
   The outline lists real prompts only, so these markers are protocol facts to
   match byte-for-byte, not display copy. */
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

/** Null hides injected payloads from the user-prompt outline. */
export function codexOutlinePreview(content: string): string | null {
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

export function copyText(text: string): Promise<void> {
  return navigator.clipboard.writeText(text);
}
