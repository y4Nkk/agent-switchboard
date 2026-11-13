import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { CommandErrorLines } from "../src/app/notifications";
import type { CommandError } from "../src/api/shared";
import type { ApplicationPreparation } from "../src/api/application-preparation";
import { PreparationDetails } from "../src/components/ApplicationPreparation";
import { applyLanguagePreference } from "../src/i18n";

export function runChecks(nativeErrors: CommandError[]) {
  assert.equal(nativeErrors.length, 7);
  let cases = 0;
  for (const language of ["zh-CN", "en-US"] as const) {
    applyLanguagePreference(language);
    const detail = "platform entry missing";
    const translated = renderToStaticMarkup(<CommandErrorLines error={{
      code: "updater-check-failed", message: detail,
      messageKey: "errors.updater.checkFailed", params: { detail },
    }} />);
    assert.equal(translated.split(detail).length - 1, 1);
    assert.doesNotMatch(translated, /asb-scope-note/);
    cases++;

    const fallback = renderToStaticMarkup(<CommandErrorLines error={{
      code: "fixture", message: detail, messageKey: "missing-catalog-key",
    }} />);
    assert.equal(fallback.split(detail).length - 1, 1);
    cases++;

    const distinct = renderToStaticMarkup(<CommandErrorLines error={{
      code: "fixture", message: detail, messageKey: "common.operationFailed",
    }} />);
    assert.match(distinct, /asb-scope-note/);
    assert.equal(distinct.split(detail).length - 1, 1);
    cases++;

    for (const error of nativeErrors) {
      const escaped = renderToStaticMarkup(<>{error.message}</>);
      const notification = renderToStaticMarkup(<CommandErrorLines error={error} />);
      assert.equal(notification.split(escaped).length - 1, 1);
      assert.doesNotMatch(notification, /private-credential-value|查看应用数据检查详情/);
      cases++;
      const snapshot: ApplicationPreparation = {
        sessionId: "isolated-error-feedback", runId: 1, revision: 1, status: "blocked", stage: "complete",
        elapsedMs: 0, settings: null, configurationReady: false, preferencesReset: false,
        repairedFiles: [], backupPaths: [], issues: [{ ...error, messageKey: "common.operationFailed" }],
      };
      const preparation = renderToStaticMarkup(<PreparationDetails snapshot={snapshot} />);
      assert.match(preparation, /asb-scope-note/);
      assert.equal(preparation.split(escaped).length - 1, 1);
      cases++;
    }
  }
  return { cases, nativeErrors: nativeErrors.length, languages: ["zh-CN", "en-US"], nativeWindowAccessed: false };
}
