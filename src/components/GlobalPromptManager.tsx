import { useEffect, useLayoutEffect, useRef } from "react";
import type { AppKind, GlobalPromptDocument } from "../api/client";
import { clientName } from "../lib/client-name";
import { ClientLogo } from "./ClientLogo";
import { Textarea } from "./Textarea";

interface GlobalPromptManagerProps {
  app: AppKind;
  document: GlobalPromptDocument | undefined;
  draft: string;
  dirty: boolean;
  busy: boolean;
  onSelectApp: (app: AppKind) => void;
  onChange: (content: string) => void;
  onSave: () => void;
  onDiscard: () => void;
  onReload: () => void;
}

function pixelValue(value: string): number | null {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function fitPromptEditor(textarea: HTMLTextAreaElement) {
  const style = window.getComputedStyle(textarea);
  const minimum = pixelValue(style.minHeight);
  const maximum = pixelValue(style.maxHeight);
  if (minimum === null || maximum === null) return;

  textarea.style.height = `${minimum}px`;
  const contentHeight = textarea.scrollHeight + textarea.offsetHeight - textarea.clientHeight;
  const height = Math.min(Math.max(contentHeight, minimum), maximum);
  textarea.style.height = `${height}px`;
  textarea.style.overflowY = contentHeight > maximum ? "auto" : "hidden";
}

/**
 * Direct editor for the two supported user-global instruction files. It has
 * no prompt-template store: the file itself is the single source of truth.
 */
export function GlobalPromptManager({
  app,
  document,
  draft,
  dirty,
  busy,
  onSelectApp,
  onChange,
  onSave,
  onDiscard,
  onReload,
}: GlobalPromptManagerProps) {
  const fileName = document?.fileName ?? "全局文档";
  const editorRef = useRef<HTMLTextAreaElement>(null);

  useLayoutEffect(() => {
    if (editorRef.current) fitPromptEditor(editorRef.current);
  }, [draft, fileName]);

  useEffect(() => {
    const fit = () => {
      if (editorRef.current) fitPromptEditor(editorRef.current);
    };
    window.addEventListener("resize", fit);
    return () => window.removeEventListener("resize", fit);
  }, []);

  return (
    <section className="asb-prompt-manager" aria-label="提示词管理">
      <div className="asb-prompt-manager-heading">
        <div>
          <h3 className="asb-prompt-manager-title">全局提示词</h3>
        </div>
        <div className="asb-prompt-document-tabs" role="tablist" aria-label="全局提示词文档">
          {(["codex", "claude"] as const).map((target) => (
            <button
              key={target}
              type="button"
              role="tab"
              aria-selected={app === target}
              className={`asb-prompt-document-tab${app === target ? " is-on" : ""}`}
              disabled={busy}
              onClick={() => onSelectApp(target)}
            >
              <ClientLogo app={target} className="asb-tab-logo" />
              <span>{clientName(target)}</span>
            </button>
          ))}
        </div>
      </div>
      <label className="asb-prompt-editor-field">
        <span className="asb-prompt-file-name">{fileName}</span>
        <Textarea
          ref={editorRef}
          code
          className="asb-input asb-code asb-textarea asb-prompt-editor"
          aria-label={`${fileName} 内容`}
          value={draft}
          disabled={busy || !document}
          placeholder={document ? `在这里编写 ${fileName}。` : "正在读取文档。"}
          onChange={(event) => onChange(event.target.value)}
        />
      </label>
      <div className="asb-prompt-actions">
        <button
          type="button"
          className="asb-btn-secondary"
          disabled={busy || dirty}
          onClick={onReload}
        >
          重新读取
        </button>
        {dirty && (
          <button type="button" className="asb-btn-secondary" disabled={busy} onClick={onDiscard}>
            放弃草稿
          </button>
        )}
        <button type="button" className="asb-btn-primary" disabled={busy || !document || !dirty} onClick={onSave}>
          {busy ? "保存中" : `保存 ${fileName}`}
        </button>
      </div>
    </section>
  );
}
