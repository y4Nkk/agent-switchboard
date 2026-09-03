import { useEffect, useLayoutEffect, useRef } from "react";
import type { GlobalPromptDocument } from "../api/client";
import { Button } from "./Button";
import { Textarea } from "./Textarea";

interface GlobalPromptManagerProps {
  document: GlobalPromptDocument | undefined;
  draft: string;
  dirty: boolean;
  busy: boolean;
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
 * The edited client follows the page-level client tabs.
 */
export function GlobalPromptManager({
  document,
  draft,
  dirty,
  busy,
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
    <section className="asb-prompt-manager" aria-label="全局指令">
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
        <Button
          variant="secondary"
          disabled={busy || dirty}
          onClick={onReload}
        >
          重新读取
        </Button>
        {dirty && (
          <Button variant="secondary" disabled={busy} onClick={onDiscard}>
            放弃草稿
          </Button>
        )}
        <Button variant="primary" disabled={busy || !document || !dirty} onClick={onSave}>
          {busy ? "保存中" : `保存 ${fileName}`}
        </Button>
      </div>
    </section>
  );
}
