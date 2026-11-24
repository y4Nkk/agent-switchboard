import { useState } from "react";
import type {
  AppKind,
  CcSwitchImportOutcome,
  CcSwitchScan,
  CommandError,
  DiscoveryReport,
  ProviderSqlImportOutcome,
  ProviderSqlScan,
} from "../api/client";
import { Tabs } from "../components/Tabs";
import { WorkspaceSubpage } from "../components/WorkspaceSubpage";
import { CcSwitchImport } from "./provider-import/CcSwitchImport";
import { LocalConfigImport } from "./provider-import/LocalConfigImport";
import { SqlExport } from "./provider-import/SqlExport";
import { SqlFileImport } from "./provider-import/SqlFileImport";
import { useI18n } from "../i18n";
import "../styles/base/provider-workspace.css";

interface ProviderImportPageProps {
  appFilter: AppKind;
  discovery: DiscoveryReport | null;
  ccScan: CcSwitchScan | null;
  ccSelected: Record<string, boolean>;
  ccResult: CcSwitchImportOutcome | null;
  ccDirectory: string | null;
  sqlScan: ProviderSqlScan | null;
  sqlSelected: Record<string, boolean>;
  sqlResult: ProviderSqlImportOutcome | null;
  busy: boolean;
  onBack: () => void;
  onScanLocal: () => void;
  onImportLocal: () => Promise<boolean>;
  onScanCc: () => void;
  onSelectCc: (key: string, checked: boolean) => void;
  onCcDirectory: (directory: string | null) => void;
  onImportCc: () => Promise<boolean>;
  onApplySql: (path: string) => void;
  onSelectSql: (key: string, checked: boolean) => void;
  onImportSql: () => Promise<boolean>;
  onError: (error: CommandError) => void;
}

/** Local discovery uses the selected client; batch import stays cross-client. */
export function ProviderImportPage(props: ProviderImportPageProps) {
  const { t } = useI18n();
  const [source, setSource] = useState<"local" | "ccswitch" | "sql" | "export">("local");
  const importLocal = async () => {
    if (await props.onImportLocal()) props.onBack();
  };
  const importCc = async () => {
    if (await props.onImportCc()) props.onBack();
  };
  const importSql = async () => {
    if (await props.onImportSql()) props.onBack();
  };
  return (
    <WorkspaceSubpage
      layout="page"
      title={t("importDiscovery.page.title")}
      backLabel={t("importDiscovery.page.back")}
      busy={props.busy}
      onBack={props.onBack}
      primary={
        <Tabs value={source} onChange={setSource} scope="provider-import" label={t("importDiscovery.page.tabsAria")}
          tabs={[
            { value: "local", label: t("importDiscovery.tab.local"), controls: "provider-import-local-panel", disabled: props.busy },
            { value: "ccswitch", label: t("importDiscovery.tab.database"), controls: "provider-import-ccswitch-panel", disabled: props.busy },
            { value: "sql", label: t("importDiscovery.tab.sqlImport"), controls: "provider-import-sql-panel", disabled: props.busy },
            { value: "export", label: t("importDiscovery.tab.sqlExport"), controls: "provider-import-export-panel", disabled: props.busy },
          ]} />
      }
    >
      <div className="asb-provider-import">
        <div id="provider-import-local-panel" role="tabpanel" aria-labelledby="provider-import-local-tab"
          hidden={source !== "local"}>
          <LocalConfigImport app={props.appFilter} discovery={props.discovery} busy={props.busy}
            onScan={props.onScanLocal} onImport={() => void importLocal()} />
        </div>
        <div id="provider-import-ccswitch-panel" role="tabpanel" aria-labelledby="provider-import-ccswitch-tab"
          hidden={source !== "ccswitch"}>
          <CcSwitchImport scan={props.ccScan} selected={props.ccSelected} result={props.ccResult}
            busy={props.busy} directory={props.ccDirectory} onScan={props.onScanCc} onSelect={props.onSelectCc}
            onDirectoryChange={props.onCcDirectory} onImport={() => void importCc()} />
        </div>
        <div id="provider-import-sql-panel" role="tabpanel" aria-labelledby="provider-import-sql-tab"
          hidden={source !== "sql"}>
          <SqlFileImport scan={props.sqlScan} selected={props.sqlSelected} result={props.sqlResult}
            busy={props.busy} onApply={props.onApplySql} onSelect={props.onSelectSql}
            onImport={() => void importSql()} />
        </div>
        <div id="provider-import-export-panel" role="tabpanel" aria-labelledby="provider-import-export-tab"
          hidden={source !== "export"}>
          <SqlExport busy={props.busy} onError={props.onError} />
        </div>
      </div>
    </WorkspaceSubpage>
  );
}
