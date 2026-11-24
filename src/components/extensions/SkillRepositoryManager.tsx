import { useState } from "react";
import { Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import type { SkillRepository } from "../../api/extensions/skill-sources";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { Tooltip } from "../Tooltip";
import { ModuleHeader } from "../WorkspaceHeader";
import { useUnsavedChanges } from "../useUnsavedChanges";
import { SkillRepositoryForm } from "./SkillRepositoryForm";
import { SkillSourceLink } from "./SkillSourceCandidate";
import { skillRepositoryName, skillSourceUrl } from "./skill-repository-model";
import type { SkillSourceState } from "./useSkillSource";

function RepositoryRow({ repo, count, failed, busy, removing, onEdit, onSave, onRemove, onConfirm, onCancel }: {
  repo: SkillRepository; count: number | null; failed: boolean; busy: boolean; removing: boolean;
  onEdit: () => void; onSave: (enabled: boolean) => void; onRemove: () => void;
  onConfirm: () => void; onCancel: () => void;
}) {
  const { t } = useI18n();
  const name = skillRepositoryName(repo.repo) ?? t("extensions.repo.fallbackName");
  return (
    <li className="asb-skill-repository-row" aria-label={name}>
      <div className="asb-skill-repository-info">
        <h3 className="asb-group-title">{name}</h3>
        <p className="asb-skill-source-origin">
          <span>{repo.refName || t("extensions.repo.defaultRef")} · {repo.subpath || t("extensions.repo.wholeRepo")}</span>
          <span className="asb-num">{count === null ? t("extensions.repo.notScanned") : t("extensions.repo.skillCount", { count })}</span>
          {failed && <span className="asb-warn-text">{t("extensions.repo.refreshFailed")}</span>}
        </p>
      </div>
      <div className="asb-skill-repository-actions">
        <Checkbox label={t("extensions.repo.enableShort")} ariaLabel={t("extensions.repo.enableAria", { name })} checked={repo.enabled} disabled={busy} onChange={onSave} />
        <SkillSourceLink url={skillSourceUrl(repo.repo)} label={t("extensions.repo.viewAria", { name })} />
        <Tooltip label={t("extensions.repo.editAria", { name })}>
          <Button variant="icon" disabled={busy} aria-label={t("extensions.repo.editAria", { name })} onClick={onEdit}><Pencil size={16} /></Button>
        </Tooltip>
        <Tooltip label={t("extensions.repo.removeAria", { name })}>
          <Button variant="icon" disabled={busy} aria-label={t("extensions.repo.removeAria", { name })} onClick={onRemove}><Trash2 size={16} /></Button>
        </Tooltip>
      </div>
      {removing && <div className="asb-skill-repository-remove">
        <p>{t("extensions.repo.removeNote")}</p>
        <Button variant="secondary" disabled={busy} onClick={onCancel}>{t("extensions.repo.cancelRemove")}</Button>
        <Button variant="danger" disabled={busy} onClick={onConfirm}><Trash2 size={16} />{t("extensions.repo.confirmRemove")}</Button>
      </div>}
    </li>
  );
}

export function SkillRepositoryManager({ state, busy }: { state: SkillSourceState; busy: boolean }) {
  const { t } = useI18n();
  const [editing, setEditing] = useState<SkillRepository | null>(null);
  const [formOpen, setFormOpen] = useState(state.repositories.ready && state.repositories.items.length === 0);
  const [removing, setRemoving] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const { repositories, catalog } = state;
  const disabled = busy || !repositories.ready || repositories.loading || state.loading || state.imports.busy;
  const leave = useUnsavedChanges(formOpen && dirty, repositories.saving);
  return (
    <section className="asb-skill-repository-manager" aria-label={t("extensions.repo.title")}>
      <ModuleHeader
        title={t("extensions.repo.title")}
        primary={
          <p className="asb-scope-note">
            {t("extensions.repo.summaryLead")}<span className="asb-num">{repositories.items.length}</span>{t("extensions.repo.summaryTail")}
          </p>
        }
        primaryActions={
          <>
            <Button variant="secondary" disabled={disabled || formOpen} onClick={() => { setEditing(null); setFormOpen(true); }}>
              <Plus size={16} />{t("extensions.repo.add")}
            </Button>
            <Button variant="secondary" disabled={repositories.saving} onClick={() => leave.request(() => state.setManagerOpen(false))}>
              {t("extensions.repo.backToSearch")}
            </Button>
          </>
        }
      />
      {repositories.error && <div className="asb-skill-source-error" role="alert">
        <span>{repositories.error}</span>
        <Button variant="secondary" disabled={busy || repositories.loading || state.imports.busy}
          onClick={() => void repositories.reload()}>
          <RefreshCw size={16} />{t("extensions.repo.retryLoad")}
        </Button>
      </div>}
      {formOpen && <SkillRepositoryForm key={editing?.id ?? "new"} initial={editing} busy={disabled}
        onDirtyChange={setDirty}
        onSave={repositories.save} onCancel={() => { setFormOpen(false); setEditing(null); }} />}
      {repositories.loading && <p className="asb-skill-source-count" role="status">{t("extensions.repo.loading")}</p>}
      {!repositories.loading && repositories.ready && repositories.items.length === 0 &&
        <div className="asb-skill-source-empty"><p>{t("extensions.repo.none")}</p></div>}
      <ul className="asb-skill-repository-list" aria-label={t("extensions.repo.listAria")}>
        {repositories.items.map((repo) => <RepositoryRow key={repo.id} repo={repo} count={catalog.count(repo.id)}
          failed={catalog.failedRepositoryIds.has(repo.id)}
          busy={disabled} removing={removing === repo.id}
          onEdit={() => { if (formOpen && editing?.id === repo.id) return;
            leave.request(() => { setEditing(repo); setFormOpen(true); setRemoving(null); }); }}
          onSave={(enabled) => void repositories.save({ ...repo, enabled })}
          onRemove={() => setRemoving(repo.id)} onCancel={() => setRemoving(null)}
          onConfirm={() => { void repositories.remove(repo.id).then((saved) => { if (saved) setRemoving(null); }); }} />)}
      </ul>
      {leave.confirmation}
    </section>
  );
}
