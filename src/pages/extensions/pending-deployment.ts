import type { AppKind, ExtensionListItem, ExtensionTarget, PlanRequestOperation } from "../../api/client";
import { targetValue } from "../../components/extensions/labels";

export function pendingDeployment(
  operations: PlanRequestOperation[], item: ExtensionListItem, scope: AppKind | ExtensionTarget,
): boolean {
  const matches = (target: ExtensionTarget) => typeof scope === "string"
    ? target.client === scope : targetValue(target) === targetValue(scope);
  return operations.some((operation) => {
    if (operation.bindingId) return item.bindings.some((binding) =>
      binding.id === operation.bindingId && matches(binding.target));
    return operation.definitionId === item.id &&
      (operation.targets?.some(matches) ?? true);
  });
}
