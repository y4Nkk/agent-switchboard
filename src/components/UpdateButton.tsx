import { UpdateIcon } from "./icons";
import { Tooltip } from "./Tooltip";

/** Topbar indicator rendered only while a newer release is known. Stateless
 * by design: the check result stays with App; clicking leads to the settings
 * page, where the release link lives. */
export function UpdateButton({
  latestVersion,
  onOpen,
}: {
  latestVersion: string;
  onOpen: () => void;
}) {
  const label = `发现新版本 ${latestVersion}`;
  return (
    <Tooltip label={label} side="bottom">
      <span className="asb-tooltip-anchor">
        <button
          type="button"
          className="asb-winbtn asb-updatebtn"
          aria-label={label}
          onClick={onOpen}
        >
          <UpdateIcon />
        </button>
      </span>
    </Tooltip>
  );
}
