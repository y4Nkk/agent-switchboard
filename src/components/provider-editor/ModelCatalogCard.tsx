import type { ReactNode } from "react";
import type { AppKind } from "../../api/client";
import { useI18n } from "../../i18n";
import { Button } from "../Button";

function ModelCatalogArt() {
  return <svg className="asb-model-catalog-art" viewBox="0 0 72 52" aria-hidden="true" focusable="false">
    <path className="asb-model-catalog-base" d="m8 35 25-16a6 6 0 0 1 5-1l25 8a3 3 0 0 1 1 5L40 47a7 7 0 0 1-5 1L9 40a3 3 0 0 1-1-5Z" />
    <path className="asb-model-catalog-shell" d="m8 27 25-16a6 6 0 0 1 5-1l25 8a3 3 0 0 1 1 5L40 39a7 7 0 0 1-5 1L9 32a3 3 0 0 1-1-5Z" />
    <path className="asb-model-catalog-surface" d="m19 25 15-10a4 4 0 0 1 4 0l14 4a2 2 0 0 1 1 4L38 33a4 4 0 0 1-4 0l-14-4a2 2 0 0 1-1-4Z" />
    <path className="asb-model-catalog-traces" d="m27 26 9-6 8 3m-13 6 9-6m7 10 5-3m-17 13 5 1 12-8" />
    <circle className="asb-model-catalog-terminal" cx="44" cy="23" r="1.8" />
  </svg>;
}

interface Props {
  app: AppKind;
  model: string;
  primary: boolean;
  busy: boolean;
  onPrimary: () => void;
  onRemove: () => void;
  sourceLabel?: string;
  children?: ReactNode;
}

export function ModelCatalogCard({ app, model, primary, busy, onPrimary, onRemove, sourceLabel, children }: Props) {
  const { t } = useI18n();
  return <section className="asb-provider-model-card" data-app={app} data-primary={primary || undefined}
    aria-label={app === "codex" ? t("providerSetup.modelCapabilities", { model }) : model}>
    <div className="asb-provider-model-card-head">
      <div className="asb-model-catalog-identity">
        <ModelCatalogArt />
        <div className="asb-model-catalog-name">
          <code>{model}</code>
          {sourceLabel && <span className="asb-field-help">{sourceLabel}</span>}
        </div>
      </div>
      {primary && <span className="asb-provider-primary-pill">{t("providerSetup.primary")}</span>}
    </div>
    {children}
    <div className="asb-model-catalog-actions">
      {!primary && <Button variant="ghost" size="compact" disabled={busy} onClick={onPrimary}>
        {t("providerSetup.usePrimary")}
      </Button>}
      <Button variant="dangerText" size="compact" disabled={busy} aria-label={t("codex.model.deleteModel", { model })}
        onClick={onRemove}>{t("providerSetup.remove")}</Button>
    </div>
  </section>;
}
