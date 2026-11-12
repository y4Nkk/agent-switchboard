import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import App from "../src/App";
import type { ApplicationPreparation as Snapshot } from "../src/api/application-preparation";
import { ApplicationPreparation, PreparationDetails } from "../src/components/ApplicationPreparation";
import { applyLanguagePreference } from "../src/i18n";

export function runChecks(report: Omit<Snapshot, "sessionId" | "runId" | "revision" | "status" | "stage" | "elapsedMs">) {
  const snapshot: Snapshot = { ...report, sessionId: "headless-ui-fixture", runId: 1, revision: 4, status: "running", stage: "configuration", elapsedMs: 22000 };
  const screen = (value: Snapshot | null) => renderToStaticMarkup(
    <ApplicationPreparation snapshot={value} error={null} onRecheck={() => {}} />,
  );
  applyLanguagePreference("zh-CN");
  const chinese = screen(snapshot);
  assert.match(chinese, /正在准备 Agent Switchboard/);
  assert.match(chinese, /正在检查并恢复应用数据/);
  assert.match(chinese, /已用 0 分 22 秒/);
  assert.match(chinese, /asb-preparation-progress/);
  assert.doesNotMatch(chinese.replace(/<[^>]*>/g, ""), /一键.*修复|查看日志|%/);

  applyLanguagePreference("en-US");
  const english = screen(snapshot);
  assert.match(english, /Preparing Agent Switchboard/);
  assert.match(english, /Checking and recovering application data/);
  assert.match(english, /Elapsed: 0 min 22 sec/);

  const failure = screen({ ...snapshot, status: "blocked", stage: "complete",
    issues: [{ code: "fixture", message: "retain <script>source</script> & backup" }] });
  assert.match(failure, /Startup preparation could not finish/);
  assert.match(failure, /Check again/);
  assert.match(failure, /View logs/);
  assert.match(failure, /&lt;script&gt;source&lt;\/script&gt;/);
  assert.doesNotMatch(failure, /asb-preparation-progress|<script>/);

  const details = renderToStaticMarkup(<PreparationDetails snapshot={{ ...snapshot,
    status: "degraded", repairedFiles: ["settings.json"], backupPaths: ["sandbox/original.json"],
    issues: [{ code: "fixture", message: "Proxy settings remain unavailable" }] }} />);
  assert.match(details, /Automatically repaired 1 files/);
  assert.match(details, /sandbox\/original.json/);
  assert.match(details, /Proxy settings remain unavailable/);

  const unknown = screen(null);
  assert.match(unknown, /asb-settings-skeleton/);
  assert.doesNotMatch(unknown, /Preparing Agent Switchboard/);
  const initialApp = renderToStaticMarkup(<App />);
  assert.match(initialApp, /asb-preparation/);
  assert.doesNotMatch(initialApp, /asb-shell|asb-workspace/);
  return { assertions: 20, languages: ["zh-CN", "en-US"], browserLaunched: false,
    checked: ["running stage and elapsed time", "blocked diagnostics", "escaped details", "partial repair report",
      "language bootstrap skeleton", "business tree deferred until preparation"] };
}
