import type { FilePreview } from "../api/client";
import { CodePreview } from "./CodePreview";
import { DiffView } from "./DiffView";

interface Props {
  filePreview: FilePreview | null;
}

/**
 * The diff inspector. Configuration text stays high-contrast on a near-solid
 * backdrop: no backdrop blur touches this panel (DESIGN.md §6). Preserved
 * host keys are not listed flat — they stay visible in place inside the
 * pretty-printed candidate file.
 */
export function PreviewInspector({ filePreview }: Props) {
  if (!filePreview) {
    return <p className="asb-empty">选择供应商后生成预览</p>;
  }
  const { preview } = filePreview;
  return (
    <div className="asb-inspector">
      {preview.changes.length === 0 && <p className="asb-empty">无变更</p>}
      <DiffView changes={preview.changes} label="变更键" />
      {preview.warnings.length > 0 && (
        <ul className="asb-warnings" aria-label="警告">
          {preview.warnings.map((warning) => (
            <li key={warning}>{warning}</li>
          ))}
        </ul>
      )}
      <CodePreview target={preview.target} content={filePreview.content} />
      <div className="asb-kv">
        <span className="asb-kv-label">备份位置</span>
        <span className="asb-kv-value asb-code">{preview.backupDir}</span>
      </div>
    </div>
  );
}
