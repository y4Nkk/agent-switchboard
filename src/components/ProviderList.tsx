import type { ProviderProfile } from "../api/client";

interface Props {
  profiles: ProviderProfile[];
  /** Profile id the live file actually matches, when the app can tell. */
  activeProfileId: string | null;
  selectedId: string | null;
  onSelect: (id: string) => void;
}

/** macOS-settings-style provider rows (DESIGN.md §8). */
export function ProviderList({ profiles, activeProfileId, selectedId, onSelect }: Props) {
  if (profiles.length === 0) {
    return <p className="asb-empty">尚无供应商</p>;
  }
  return (
    <ul className="asb-rows" role="listbox" aria-label="供应商列表">
      {profiles.map((profile) => {
        const active = profile.id === activeProfileId;
        return (
          <li key={profile.id}>
            <button
              type="button"
              role="option"
              aria-selected={selectedId === profile.id}
              className={`asb-row${selectedId === profile.id ? " is-selected" : ""}`}
              onClick={() => onSelect(profile.id)}
            >
              <span className={`asb-dot${active ? " is-on" : ""}`} aria-hidden="true" />
              <span className="asb-row-main">
                <span className="asb-row-name">
                  {profile.name}
                  {active && <span className="asb-row-active">当前</span>}
                </span>
                <span className="asb-row-meta">{profile.model ?? "默认模型"}</span>
              </span>
              <span className="asb-row-endpoint">
                {profile.mode === "official" ? "官方登录" : "自定义服务"}
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
