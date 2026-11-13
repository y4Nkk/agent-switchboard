import { useEffect, useRef, useState, type RefObject } from "react";
import type { AppKind } from "../../api/client";
import type { ProviderEditorContext, ProviderEditorSection, ProviderEditorNavigation as RequestNavigation } from "../../app/provider-editor-session";
import type { RouteRole } from "../model-routing/model";
import { useUnsavedChanges } from "../useUnsavedChanges";

export type { ProviderEditorSection } from "../../app/provider-editor-session";

export interface ProviderEditorNavigation extends RequestNavigation, Pick<ProviderEditorContext, "profileId" | "section" | "returnTo"> {
  routeRole?: RouteRole;
}

export function useProviderEditorSections(props: ProviderEditorNavigation & {
  app: AppKind; official: boolean; dirty: boolean; busy: boolean; active: boolean;
  viewportRef: RefObject<HTMLDivElement | null>;
}) {
  const [section, updateSection] = useState<ProviderEditorSection>(
    props.official && props.section === "models" ? "connection" : props.section,
  );
  const [role, setRole] = useState<RouteRole>(props.routeRole ?? "main");
  const positions = useRef<Partial<Record<ProviderEditorSection, number>>>({});
  const seen = useRef(0);
  const current = (props.app === "codex" ? props.inventory.codex.find(({ profile }) => profile.id === props.profileId) : null)
    ?? props.inventory.profiles.find(({ profile }) => profile.app === props.app && profile.id === props.profileId);
  const originalHash = useRef(current?.fileHash ?? null);
  const stale = originalHash.current !== null && current?.fileHash !== originalHash.current;
  const leave = useUnsavedChanges(props.dirty, props.busy);
  const selectSection = (next: ProviderEditorSection) => {
    positions.current[section] = props.viewportRef.current?.scrollTop ?? 0;
    updateSection(props.official && next === "models" ? "connection" : next);
  };
  const focusInspector = () => requestAnimationFrame(() => {
    const inspector = props.viewportRef.current?.querySelector<HTMLElement>(".asb-model-route-inspector");
    inspector?.scrollIntoView({ block: "nearest" });
    inspector?.querySelector<HTMLElement>("input,button")?.focus({ preventScroll: true });
  });
  useEffect(() => {
    props.viewportRef.current?.scrollTo({ top: positions.current[section] ?? 0 });
  }, [section, props.viewportRef]);
  useEffect(() => {
    if (props.section !== "models" || props.official || !props.active) return;
    const frame = focusInspector();
    return () => cancelAnimationFrame(frame);
  }, [props.profileId]);
  useEffect(() => {
    const request = props.request;
    if (!props.active || props.busy || !request || seen.current === request.revision) return;
    seen.current = request.revision;
    props.onRequestHandled(request.revision);
    if (!request.reload && request.app === props.app && request.profileId === props.profileId) {
      selectSection(request.section);
      setRole(request.role ?? "main");
      if (request.section === "models" && !props.official) focusInspector();
    } else leave.request(() => props.onReplaceSession(request));
  }, [props.request, props.busy, props.active]);
  return { section, selectSection, role, setRole, leave, stale };
}

/** Native constraints are checked after revealing their owning tab. */
export function validateEditorForm(form: HTMLFormElement, selectSection: (section: ProviderEditorSection) => void) {
  const invalid = [...form.elements].find((element) =>
    (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement)
      && !element.disabled && !element.validity.valid,
  );
  if (!(invalid instanceof HTMLInputElement || invalid instanceof HTMLTextAreaElement || invalid instanceof HTMLSelectElement)) return true;
  const section = invalid.closest<HTMLElement>("[data-editor-section]")?.dataset.editorSection as ProviderEditorSection | undefined;
  if (section) selectSection(section);
  requestAnimationFrame(() => invalid.reportValidity());
  return false;
}
