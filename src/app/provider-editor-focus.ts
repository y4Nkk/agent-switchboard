import type { ProviderEditorSection } from "./provider-editor-session";

export function restoreProviderEditorFocus(profileId: string, section: ProviderEditorSection) {
  requestAnimationFrame(() => {
    const key = section === "models" ? "providerRoutingId" : "providerEditId";
    const controls = document.querySelectorAll<HTMLButtonElement>("[data-provider-routing-id], [data-provider-edit-id]");
    [...controls].find((button) => button.dataset[key] === profileId && button.getClientRects().length > 0)?.focus();
  });
}
