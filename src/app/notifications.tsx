import type { AppKind } from "../api/client";
import { toast } from "../components/use-toast";
import { clientName } from "../lib/client-name";

/** Warning list shared by every write path: one warning toast, each line its
 * own row (DESIGN.md §8 全局通知). */
export function notifyWarnings(title: string, warnings: string[]) {
  toast({
    kind: "warning",
    title: `${title}，有 ${warnings.length} 条警告`,
    description: (
      <>
        {warnings.map((warning) => (
          <div key={warning}>{warning}</div>
        ))}
      </>
    ),
  });
}

/** One-shot write results (switch / restore / undo) report through the global
 * toaster; the banner layer stays reserved for persistent errors that carry
 * a decision (DESIGN.md §7). */
export function notifyWriteOutcome(title: string, app: AppKind, warnings: string[]) {
  if (warnings.length > 0) {
    notifyWarnings(title, warnings);
    return;
  }
  toast({
    kind: "success",
    title,
    description: `${clientName(app)}将在下次启动时读取新配置`,
  });
}
