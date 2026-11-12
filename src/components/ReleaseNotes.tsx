import { useI18n } from "../i18n";
import { MarkdownContent } from "./Markdown";

export function ReleaseNotes({ notes, version }: { notes: string; version: string }) {
  const { t } = useI18n();
  return <div className="asb-update-notes asb-markdown" aria-label={t("backup.update.notesAria", { version })}>
    <MarkdownContent text={notes} />
  </div>;
}
