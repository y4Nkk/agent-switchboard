import { useEffect, useRef, useState } from "react";
import type { SkillVersion } from "../../api/client";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { ConfirmSheet } from "../ConfirmSheet";
import { Input } from "../Input";
import { Select } from "../Select";
import { Textarea } from "../Textarea";
import type { useSkillDependencies, useSkillFiles } from "./useSkillWorkbench";

function formatBytes(size: number) {
  return size < 1024 ? `${size} B` : size < 1024 * 1024 ? `${(size / 1024).toFixed(1)} KB` : `${(size / 1024 / 1024).toFixed(1)} MB`;
}

export function SkillFiles({ files, busy }: { files: ReturnType<typeof useSkillFiles>; busy: boolean }) {
  const { t } = useI18n();
  const [path, setPath] = useState("");
  const document = files.document;
  if (!document) return null;
  const validPath = path.trim() !== "" && !files.paths.includes(path.trim());
  return <section className="asb-editor-section">
    <p className="asb-scope-note">{t("extensions.workbench.revisionLead", { revision: document.view.revision })}
      <code>{document.view.contentDigest.slice(0, 12)}</code>{t("extensions.workbench.revisionTail")}</p>
    <ul className="asb-ext-binding-list">{files.paths.map((name) => {
      const stored = document.view.files.find((file) => file.relativePath === name);
      const binary = stored?.text === null;
      const modified = !binary && document.drafts[name] !== stored?.text;
      return <li key={name} className={`asb-ext-binding asb-skill-file${files.selected === name ? " is-active" : ""}`}>
        <Button variant="secondary" disabled={busy || binary}
        aria-pressed={files.selected === name} onClick={() => files.select(name)}><code>{name}</code></Button>
        {files.selected === name && <span className="asb-pill-status">{t("extensions.workbench.currentFile")}</span>}
        {modified && <span className="asb-scope-note">{t("extensions.workbench.modified")}</span>}
        {binary && stored && <span className="asb-pill-status">{t("extensions.workbench.binary", { size: formatBytes(stored.size) })}</span>}</li>;
    })}</ul>
    <div className="asb-ext-actions asb-skill-file-create"><Input value={path} disabled={busy} aria-label={t("extensions.workbench.newPathAria")}
      placeholder={t("extensions.workbench.newPathPlaceholder")} onChange={(event) => setPath(event.target.value)} />
      <Button variant="secondary" disabled={busy || !validPath} onClick={() => {
        const name = path.trim(); files.change(name, ""); files.select(name); setPath("");
      }}>{t("extensions.workbench.addFile")}</Button></div>
    {files.selected && document.drafts[files.selected] !== undefined && <Textarea code rows={18} disabled={busy}
      aria-label={t("extensions.workbench.editFileAria", { file: files.selected })} value={document.drafts[files.selected]}
      onChange={(event) => { if (files.selected) files.change(files.selected, event.target.value); }} />}
  </section>;
}

export function SkillVersions({ id, revision, busy, onLoad, onRestore }: { id: string; revision: number; busy: boolean;
  onLoad: (id: string) => Promise<SkillVersion[] | null>; onRestore: (digest: string) => void }) {
  const { t } = useI18n();
  const [versions, setVersions] = useState<SkillVersion[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [pending, setPending] = useState<string | null>(null);
  const generation = useRef(0);
  useEffect(() => { generation.current += 1; setVersions(null); setPending(null); setLoading(false);
    return () => { generation.current += 1; }; }, [id, revision]);
  const load = async () => {
    if (loading || busy) return;
    const request = ++generation.current;
    setLoading(true);
    try { const next = await onLoad(id); if (request === generation.current) setVersions(next); }
    finally { if (request === generation.current) setLoading(false); }
  };
  return <section className="asb-editor-section" aria-busy={loading}>
    <Button variant="secondary" disabled={busy || loading} onClick={() => void load()}>{t("extensions.workbench.loadVersions")}</Button>
    {versions && <ul className="asb-ext-binding-list">{versions.map((version) => <li key={version.digest} className="asb-ext-binding">
      <code>{version.digest.slice(0, 12)}</code><span>{t("extensions.workbench.versionMeta", {
        count: version.fileCount, size: formatBytes(version.totalBytes),
      })}</span>{version.isCurrent ? <span className="asb-pill-status">{t("extensions.workbench.currentVersion")}</span>
        : <Button variant="secondary" disabled={busy || loading} onClick={() => setPending(version.digest)}>{t("extensions.workbench.restoreVersion")}</Button>}
    </li>)}</ul>}
    {pending && <ConfirmSheet title={t("extensions.workbench.restoreVersion")} busy={busy}
      confirmLabel={t("extensions.workbench.restoreVersion")} onCancel={() => setPending(null)}
      onConfirm={() => { const digest = pending; setPending(null); onRestore(digest); }}>
      <p>{t("extensions.workbench.restoreNote")}</p><code>{pending}</code>
    </ConfirmSheet>}
  </section>;
}

export function SkillDependencies({ deps, options, busy, onReload }: { deps: ReturnType<typeof useSkillDependencies>;
  options: Array<{ id: string; name: string }>; busy: boolean; onReload: () => void }) {
  const { t } = useI18n();
  const selectOptions = [{ value: "", label: t("extensions.workbench.unlinked") },
    ...options.map((option) => ({ value: option.id, label: option.name }))];
  const patch = (index: number, value: Partial<(typeof deps.draft)[number]>) =>
    deps.change(deps.draft.map((row, position) => position === index ? { ...row, ...value } : row));
  return <section className="asb-editor-section"><p className="asb-scope-note">{t("extensions.workbench.depsNote")}</p>
    {deps.stale && <div className="asb-banner" role="alert">{t("extensions.workbench.sourceChanged")}
      <Button variant="secondary" disabled={busy} onClick={onReload}>{t("extensions.library.reload")}</Button></div>}
    <ul className="asb-ext-binding-list">{deps.draft.map((row, index) => <li key={index} className="asb-ext-binding asb-skill-dependency">
      <Input value={row.name} aria-label={t("extensions.workbench.depNameAria")} disabled={busy}
        onChange={(event) => patch(index, { name: event.target.value })} />
      <Select value={row.resourceId ?? ""} options={selectOptions} disabled={busy}
        ariaLabel={t("extensions.workbench.depSelectAria", { row: index + 1 })}
        onChange={(value) => patch(index, { resourceId: value || null })} />
      <Button variant="secondary" disabled={busy} onClick={() => deps.change(deps.draft.filter((_, position) => position !== index))}>{t("extensions.workbench.removeDep")}</Button>
    </li>)}</ul><Button variant="secondary" disabled={busy}
      onClick={() => deps.change([...deps.draft, { name: "", resourceId: null }])}>{t("extensions.workbench.addDep")}</Button>
  </section>;
}
