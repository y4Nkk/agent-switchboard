import type { ReactNode, Ref } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { EditorFrame } from "../EditorFrame";

interface Props {
  title: string;
  titleRef?: Ref<HTMLHeadingElement>;
  /** Required: every two-level session hands the viewport to its editor hook,
   * which resets scroll when entering runtime parameters. */
  viewportRef: Ref<HTMLDivElement>;
  backLabel: string;
  busy: boolean;
  onBack: () => void;
  onCancel: () => void;
  formId?: string;
  canSave: boolean;
  children: ReactNode;
}

/** Composes the provider write bar (取消 + 保存供应商) onto the shared
 * editor frame; the save action may live in an external form via `formId`. */
export function ProviderEditorFrame({
  title,
  titleRef,
  viewportRef,
  backLabel,
  busy,
  onBack,
  onCancel,
  formId,
  canSave,
  children,
}: Props) {
  const { t } = useI18n();
  const canSubmit = Boolean(formId) && canSave && !busy;
  return (
    <EditorFrame
      title={title}
      titleRef={titleRef}
      viewportRef={viewportRef}
      backLabel={backLabel}
      busy={busy}
      onBack={onBack}
      className="asb-provider-editor"
      footer={
        <>
          <Button variant="secondary" disabled={busy} onClick={onCancel}>{t("confirm.cancel")}</Button>
          <Button type={formId ? "submit" : "button"} form={formId} variant="primary" className="asb-editor-submit" disabled={!canSubmit}>
            {t("providers.editor.save")}
          </Button>
        </>
      }
    >
      {children}
    </EditorFrame>
  );
}
