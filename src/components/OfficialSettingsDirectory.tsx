import type { OfficialSettingDirectoryEntry } from "../api/client";
import { useState } from "react";
import { useI18n } from "../i18n";
import { catalogText } from "../i18n/errors";
import { SearchInput } from "./SearchInput";

interface Props {
  entries: OfficialSettingDirectoryEntry[];
}

/**
 * The official settings directory dialog's body: a coverage map, not a second
 * configuration editor. The backend directory owns every path and its real
 * write boundary; this component only makes that boundary inspectable before
 * a user expects a project, policy, or login resource to be changed by a
 * supplier activation.
 */
export function OfficialSettingsDirectory({ entries }: Props) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const needle = query.trim().toLocaleLowerCase();
  const visible = entries.filter((entry) => [entry.title, entry.detail, ...entry.paths]
    .map((value) => catalogText(value, t)).join(" ").toLocaleLowerCase().includes(needle));
  const dispositionLabel: Record<OfficialSettingDirectoryEntry["disposition"], string> = {
    direct: t("clientConfig.directory.dispositionDirect"),
    separateModule: t("clientConfig.directory.dispositionSeparate"),
    preserveOnly: t("clientConfig.directory.dispositionPreserve"),
  };
  return (
    <div className="asb-official-directory">
      <SearchInput value={query} ariaLabel={t("clientConfig.directory.search")}
        placeholder={t("clientConfig.directory.search")} onChange={setQuery} />
      <div className="asb-official-directory-list">
        {visible.length === 0 && <p role="status">{t("clientConfig.directory.noMatches")}</p>}
        {visible.map((entry) => (
          <article className="asb-official-directory-entry" key={`${entry.title}:${entry.paths.join("|")}`}>
            <div className="asb-official-directory-entry-head">
              <h4 className="asb-group-title">{catalogText(entry.title, t)}</h4>
              <span className={`asb-official-directory-status is-${entry.disposition}`}>
                {dispositionLabel[entry.disposition]}
              </span>
            </div>
            <p className="asb-official-directory-paths">{entry.paths.map((path) => catalogText(path, t)).join(" · ")}</p>
            <p className="asb-official-directory-detail">{catalogText(entry.detail, t)}</p>
          </article>
        ))}
      </div>
    </div>
  );
}
