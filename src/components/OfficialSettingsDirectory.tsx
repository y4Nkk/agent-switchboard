import type { AppKind, OfficialSettingDirectoryEntry } from "../api/client";
import { clientName } from "../lib/client-name";

interface Props {
  app: AppKind;
  entries: OfficialSettingDirectoryEntry[];
}

const dispositionLabel: Record<OfficialSettingDirectoryEntry["disposition"], string> = {
  direct: "基础参数",
  separateModule: "独立模块",
  preserveOnly: "保留不写入",
};

/**
 * A coverage map, not a second configuration editor. The backend directory
 * owns every path and its real write boundary; this component only makes that
 * boundary inspectable before a user expects a project, policy, or login
 * resource to be changed by a supplier activation.
 */
export function OfficialSettingsDirectory({ app, entries }: Props) {
  return (
    <section className="asb-official-directory" aria-label="官方设置目录">
      <div className="asb-official-directory-heading">
        <h3 className="asb-official-directory-title">官方设置目录</h3>
        <p className="asb-official-directory-note">
          {clientName(app)} 的用户级设置、独立资源与项目/受管状态按真实所有权列出。
        </p>
      </div>
      <div className="asb-official-directory-list">
        {entries.map((entry) => (
          <article className="asb-official-directory-entry" key={`${entry.title}:${entry.paths.join("|")}`}>
            <div className="asb-official-directory-entry-head">
              <h4>{entry.title}</h4>
              <span className={`asb-official-directory-status is-${entry.disposition}`}>
                {dispositionLabel[entry.disposition]}
              </span>
            </div>
            <p className="asb-official-directory-paths">{entry.paths.join(" · ")}</p>
            <p className="asb-official-directory-detail">{entry.detail}</p>
          </article>
        ))}
      </div>
    </section>
  );
}
