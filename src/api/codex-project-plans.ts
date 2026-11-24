/*
 * Codex 项目方案（E06）：供应商 + MCP + Skills + 指令预设的命名联动快照。
 * 本模块只做类型化调用；真实写入由后端逐项交回各资源域所有者完成。
 */
import { invoke, type LocalizedMessage } from "./client";

/** 槽位语义：`null` = 从未拍过快照（应用时不动）；空数组/空串 = 拍到的就是空。 */
export interface CodexProjectSlot {
  providers: string | null;
  mcp: string[] | null;
  skills: string[] | null;
  prompts: string | null;
}

export interface CodexProjectPlan {
  id: string;
  name: string;
  slot: CodexProjectSlot;
  updatedAt: string;
}

export interface CodexProjectBindingFact {
  bindingId: string;
  definitionId: string;
  name: string;
  kind: "mcp" | "skill";
  scope: string;
  enabled: boolean;
}

export interface CodexProjectPlansView {
  plans: CodexProjectPlan[];
  current: string | null;
  revision: string;
  activeProviderId: string | null;
  activePromptId: string | null;
  bindings: CodexProjectBindingFact[];
  /** 名称由后端解析；渲染层从不自己把 id 变成可读文本以外的推断。 */
  providers: Array<{ id: string; name: string }>;
  prompts: Array<{ id: string; name: string }>;
}

export interface CodexProjectApplyStep {
  kind: "provider" | "mcp" | "skill" | "prompt";
  target: string;
  label: string;
  action: "switch" | "enable" | "disable" | "activate";
}

export interface CodexProjectApplyPreview {
  planId: string;
  steps: CodexProjectApplyStep[];
  warnings: LocalizedMessage[];
  /** 应用前会自动回拍的那个项目；`null` 表示没有需要补拍的旧项目。 */
  autosavePlanId: string | null;
}

export interface CodexProjectApplyOutcome {
  steps: CodexProjectApplyStep[];
  warnings: LocalizedMessage[];
  view: CodexProjectPlansView;
}

export const listCodexProjectPlans = (): Promise<CodexProjectPlansView> =>
  invoke("list_codex_project_plans");

export const createCodexProjectPlan = (
  name: string,
  expectedRevision: string,
): Promise<CodexProjectPlansView> =>
  invoke("create_codex_project_plan", { name, expectedRevision });

export const renameCodexProjectPlan = (
  planId: string,
  name: string,
  expectedRevision: string,
): Promise<CodexProjectPlansView> =>
  invoke("rename_codex_project_plan", { planId, name, expectedRevision });

export const resnapshotCodexProjectPlan = (
  planId: string,
  expectedRevision: string,
): Promise<CodexProjectPlansView> =>
  invoke("resnapshot_codex_project_plan", { planId, expectedRevision });

export const deleteCodexProjectPlan = (
  planId: string,
  expectedRevision: string,
): Promise<CodexProjectPlansView> =>
  invoke("delete_codex_project_plan", { planId, expectedRevision });

export const setCurrentCodexProjectPlan = (
  planId: string | null,
  expectedRevision: string,
): Promise<CodexProjectPlansView> =>
  invoke("set_current_codex_project_plan", { planId, expectedRevision });

export const previewCodexProjectPlanApply = (
  planId: string,
  expectedRevision: string,
): Promise<CodexProjectApplyPreview> =>
  invoke("preview_codex_project_plan_apply", { planId, expectedRevision });

export const applyCodexProjectPlan = (
  planId: string,
  expectedRevision: string,
  confirmWrite: boolean,
): Promise<CodexProjectApplyOutcome> =>
  invoke("apply_codex_project_plan", { planId, expectedRevision, confirmWrite });
