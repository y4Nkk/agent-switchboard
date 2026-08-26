import type { CommonConfigPatch, PatchValue } from "../api/client";

interface Props {
  patch: CommonConfigPatch;
  busy: boolean;
  onChange: (patch: CommonConfigPatch) => void;
}

function find(patch: CommonConfigPatch, key: string): PatchValue | undefined {
  return patch.entries.find((entry) => entry.key === key)?.value;
}

/**
 * General-configuration overlay form. It edits a typed patch; it never
 * produces configuration text. Model routing and run parameters belong to
 * provider profiles and are edited there.
 */
export function GeneralSettingsForm({ patch, busy, onChange }: Props) {
  const setEntry = (key: string, value: PatchValue | null) => {
    const entries = patch.entries.filter((entry) => entry.key !== key);
    if (value !== null) entries.push({ key, value });
    onChange({ ...patch, entries });
  };

  if (patch.app === "codex") {
    return (
      <form className="asb-form" aria-label="Codex 通用设置">
        <label className="asb-field asb-field-check">
          <input
            type="checkbox"
            checked={Boolean(find(patch, "disable_response_storage"))}
            disabled={busy}
            onChange={(e) =>
              setEntry("disable_response_storage", e.target.checked ? true : null)
            }
          />
          <span className="asb-kv-label">禁用响应存储</span>
        </label>
      </form>
    );
  }

  return (
    <p className="asb-empty">
      模型与服务地址由各供应商档案管理；Claude Code 暂无其他通用配置项。
    </p>
  );
}
