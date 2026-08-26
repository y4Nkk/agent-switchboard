import type { FilePreview } from "../api/client";

interface Props {
  filePreview: FilePreview | null;
}

function changeLine(before: string | null, after: string | null): string {
  const from = before ?? "（无）";
  const to = after ?? "移除";
  return `${from} → ${to}`;
}

/**
 * The diff inspector. Configuration text stays high-contrast on a near-solid
 * backdrop: no backdrop blur touches this panel (DESIGN.md §6).
 */
export function PreviewInspector({ filePreview }: Props) {
  if (!filePreview) {
    return <p className="asb-empty">选择供应商后生成预览</p>;
  }
  const { preview } = filePreview;
  return (
    <div className="asb-inspector">
      <h2 className="asb-panel-title">{preview.target}</h2>
      {preview.changes.length === 0 && <p className="asb-empty">无变更</p>}
      <ul className="asb-diff" aria-label="变更键">
        {preview.changes.map((change) => (
          <li key={change.key} className="asb-diff-row">
            <span className="asb-diff-key">{change.key}</span>
            <span className="asb-diff-value">{changeLine(change.before, change.after)}</span>
          </li>
        ))}
      </ul>
      {preview.warnings.length > 0 && (
        <ul className="asb-warnings" aria-label="警告">
          {preview.warnings.map((warning) => (
            <li key={warning}>{warning}</li>
          ))}
        </ul>
      )}
      <div className="asb-kv">
        <span className="asb-kv-label">保留的宿主键</span>
        <span className="asb-kv-value">{preview.preserved.join("、") || "（无）"}</span>
      </div>
      <div className="asb-kv">
        <span className="asb-kv-label">备份位置</span>
        <span className="asb-kv-value asb-code">{preview.backupDir}</span>
      </div>
    </div>
  );
}
