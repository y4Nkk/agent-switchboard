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

function stateCopy(document: GlobalPromptDocument | undefined, dirty: boolean): string {
  if (!document) return "正在读取全局提示词文档。";
  if (dirty) return "有未保存的草稿。保存前会核对文档是否被外部改动。";
  if (!document.exists) return "该文档尚未创建；保存后会写入客户端的全局目录。";
  return "已载入本机全局文档；项目内指令可在各自范围内追加更具体的约定。";
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
  return (
    <section className="asb-prompt-manager" aria-label="提示词管理">
      <div className="asb-prompt-manager-heading">
        <div>
          <h3 className="asb-prompt-manager-title">全局提示词</h3>
          <p className="asb-scope-note">为 Codex 和 Claude Code 分别维护跨项目生效的工作约定。</p>
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
      <p className="asb-prompt-state" role="status">
        {stateCopy(document, dirty)}
      </p>
    </section>
  );
}
