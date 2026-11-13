import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import { UpdateButton } from "../src/components/UpdateButton";
import { UpdateDownloadPanel } from "../src/components/UpdateDownloadPanel";
import { UpdateSection } from "../src/components/UpdateSection";
import { updateDownloadView } from "../src/app/update-download-state";
import { applicationUpdateView, UPDATE_PHASE_LABELS, type ApplicationUpdateSnapshot } from "../src/app/update-state";
import { applyLanguagePreference, tr } from "../src/i18n";
import { candidate, updateExamples, updateError, updateSnapshot } from "../tests/fixtures/update-state";

const noop = () => undefined;
const control = (state: ApplicationUpdateSnapshot) => ({ state, onCheck: noop, onInstall: noop, onCancel: noop, onRestart: noop });

function sidebar(state: ApplicationUpdateSnapshot, collapsed: boolean) {
  return renderToStaticMarkup(<UpdateButton update={control(state)} collapsed={collapsed} onOpen={noop} />);
}

function checkSidebar(html: string, state: ApplicationUpdateSnapshot) {
  const view = applicationUpdateView(state);
  const { download, phase } = view;
  const button = html.match(/<button[^>]*>/)?.[0] ?? "";
  const status = html.match(/<span class="asb-updatebtn-status"[^>]*>/)?.[0] ?? "";
  assert.ok(button.includes(`data-phase="${phase}"`));
  assert.ok(button.includes(`data-tone="${view.tone}"`));
  assert.equal(button.includes("disabled="), phase === "checking" || phase === "initializing");
  assert.equal(button.includes('aria-busy="true"'), view.busy);
  assert.ok(status.includes(`role="${view.busy ? "progressbar" : "status"}"`));
  const livePercent = phase === "downloading" && !download.stale ? download.percent : null;
  assert.equal(status.includes("aria-valuenow="), livePercent !== null);
  if (livePercent !== null) {
    assert.ok(status.includes(`aria-valuenow="${livePercent}"`));
    assert.ok(html.includes(`stroke-dashoffset="${100 - livePercent}"`));
    assert.ok(html.includes(`>${livePercent}%</span>`));
  } else assert.doesNotMatch(html, /class="asb-updatebtn-percent/);
  if (download.stale) {
    assert.match(html, /data-spinning="false"/);
    assert.match(html, /fixture status unavailable/);
    assert.match(html, /上次|Last/);
  }
  if (phase === "downloading" && download.percent === null) {
    assert.match(html, /data-indeterminate="true"/);
    assert.match(html, /总大小未知|total size unknown/);
    assert.doesNotMatch(status, /[0-9]+%/);
  }
  for (const error of [view.error, download.cancelError, download.readError]) {
    if (error && typeof error === "object" && "message" in error) assert.ok(button.includes(String(error.message)));
  }
  if (phase === "installed") assert.match(html, /stroke-dashoffset="0"/);
  if (["starting", "verifying", "cancelling", "installing", "restarting"].includes(phase)) assert.match(html, /data-indeterminate="true"/);
  assert.equal(html.includes('class="asb-updatebtn-dot"'), phase === "available");
}

function checkPanel(state: ApplicationUpdateSnapshot) {
  const html = renderToStaticMarkup(<UpdateDownloadPanel download={state} onCancel={noop} />);
  const view = updateDownloadView(state);
  const { phase, percent, stale } = view;
  if (!phase || phase === "installed") { assert.equal(html, ""); return; }
  assert.ok(html.includes(`data-phase="${phase}"`));
  const meter = html.match(/<div class="asb-update-download-meter"[^>]*>/)?.[0] ?? "";
  assert.equal(meter.includes("aria-valuenow="), !stale && percent !== null);
  if (percent !== null) assert.ok(html.includes(`width:${percent}%`));
  if (stale) { assert.match(html, /上次|Last/); assert.match(meter, /data-stale="true"/); }
  if (phase === "installing" || view.retry) assert.doesNotMatch(html, /<button/);
  if (phase === "cancelling") assert.match(html, /<button[^>]+disabled=""/);
  if (!state.task) assert.doesNotMatch(html, /0 B/);
  for (const error of [view.error, view.cancelError, view.readError]) {
    if (error && typeof error === "object" && "message" in error) {
      assert.match(html, /role="alert"/);
      assert.equal(html.split(String(error.message)).length - 1, 1);
    }
  }
}

function checkSection(state: ApplicationUpdateSnapshot, busy = false) {
  const html = renderToStaticMarkup(<UpdateSection update={control(state)} busy={busy} />);
  const view = applicationUpdateView(state);
  if (view.phase !== "available" && view.phase !== "storeManaged") assert.ok(html.includes(tr(UPDATE_PHASE_LABELS[view.phase])));
  const expected = new Set([view.error, view.download.error, view.download.readError, view.download.cancelError]);
  for (const error of expected) {
    if (error && typeof error === "object" && "message" in error) assert.equal(html.split(String(error.message)).length - 1, 1);
  }
  assert.ok((html.match(/class="asb-btn asb-btn-primary(?: |")/g) ?? []).length <= 1);
  if (view.phase === "storeManaged") assert.doesNotMatch(html, /<button|role="progressbar"/);
  if (view.phase === "channelFailed") assert.ok(html.includes(tr("backup.update.retryChannel")));
}

function checkPrimaryActions() {
  const start = updateSnapshot({ updateCheck: candidate, startError: updateError });
  const html = renderToStaticMarkup(<UpdateSection update={control(start)} busy={false} />);
  assert.equal((html.match(/class="asb-btn asb-btn-primary(?: |")/g) ?? []).length, 1);
  assert.equal(html.split("fixture connection refused").length - 1, 1);
  const ordinaryBusy = renderToStaticMarkup(<UpdateSection update={control(updateSnapshot())} busy />);
  assert.equal(ordinaryBusy.includes(tr("update.checking")), false);
  const checking = renderToStaticMarkup(<UpdateSection update={control(updateSnapshot({ checking: true }))} busy={false} />);
  assert.equal(checking.includes(tr("update.checking")), true);
}

export function runChecks() {
  let sidebarRenders = 0;
  let panelRenders = 0;
  let sectionRenders = 0;
  for (const language of ["zh-CN", "en-US"] as const) {
    applyLanguagePreference(language);
    for (const [, state] of updateExamples()) {
      for (const collapsed of [false, true]) { checkSidebar(sidebar(state, collapsed), state); sidebarRenders++; }
      checkPanel(state); panelRenders++;
      checkSection(state); sectionRenders++;
    }
    checkPrimaryActions(); sectionRenders += 3;
  }
  return { passed: true, sidebarRenders, panelRenders, sectionRenders, languages: ["zh-CN", "en-US"],
    nativeCommandsInvoked: false, nativeWindowAccessed: false };
}

export function previewMarkup() {
  return (["zh-CN", "en-US"] as const).map((language) => {
    applyLanguagePreference(language);
    return `<h2>${language}</h2><div class="preview-grid">${updateExamples().map(([name, state]) =>
      renderToStaticMarkup(<section className="preview-case" key={name}>
        <h3>{name}</h3>
        <div className="preview-rails">{[false, true].map((collapsed) =>
          <div className={`asb-shell preview-rail${collapsed ? " is-collapsed" : ""}`} key={String(collapsed)}>
            <ul className="asb-sidenav-footer"><li>
              <UpdateButton update={control(state)} collapsed={collapsed} onOpen={noop} />
            </li></ul>
          </div>)}
        </div>
      </section>)).join("")}</div>`;
  }).join("");
}
