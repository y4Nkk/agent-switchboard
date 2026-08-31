import { useEffect, useState } from "react";
import {
  closeWindow,
  getWindowMaximized,
  minimizeWindow,
  onWindowResized,
  toggleMaximizeWindow,
} from "../api/client";
import { CloseIcon, MaximizeIcon, MinimizeIcon, RestoreIcon } from "./icons";
import { Tooltip } from "./Tooltip";

/** Custom window controls for the undecorated, integrated title bar
 * (PC Manager-style). All Tauri access goes through the api client. The
 * maximize button mirrors the live window state: single square while
 * windowed, overlapping squares while maximized. */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let active = true;
    const sync = () => {
      void getWindowMaximized()
        .then((value) => {
          if (active) setMaximized(value);
        })
        .catch(() => {});
    };
    sync();
    const unlisten = onWindowResized(sync).catch(() => () => {});
    return () => {
      active = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  return (
    <div className="asb-wincontrols">
      <Tooltip label="最小化" side="bottom">
        <button
          type="button"
          className="asb-winbtn"
          aria-label="最小化"
          onClick={() => void minimizeWindow().catch(() => {})}
        >
          <MinimizeIcon />
        </button>
      </Tooltip>
      <Tooltip label={maximized ? "还原" : "最大化"} side="bottom">
        <button
          type="button"
          className="asb-winbtn"
          aria-label={maximized ? "还原" : "最大化"}
          onClick={() => void toggleMaximizeWindow().catch(() => {})}
        >
          {maximized ? <RestoreIcon /> : <MaximizeIcon />}
        </button>
      </Tooltip>
      <Tooltip label="关闭" side="bottom">
        <button
          type="button"
          className="asb-winbtn asb-winbtn-close"
          aria-label="关闭"
          onClick={() => void closeWindow().catch(() => {})}
        >
          <CloseIcon />
        </button>
      </Tooltip>
    </div>
  );
}
