import { useId } from "react";
import type { ResourceGroup, ResourceMetric as Metric } from "../../api/client";
import { ResourceMetric } from "./ResourceMetric";

interface Props {
  group: ResourceGroup;
  metric: Metric;
  kind: "cpu" | "memory";
  maximum: number | null;
}

/** The ring is decorative; the adjacent text owns the exact accessible reading. */
export function ResourceUsageRing({ group, metric, kind, maximum }: Props) {
  const patternId = useId();
  const available = metric.value !== null && maximum !== null;
  const portion = metric.value === null || maximum === null
    ? 0 : Math.min(100, Math.max(0, metric.value / maximum * 100));
  const partial = metric.state === "partial";
  const arc = { cx: 32, cy: 32, r: 27, pathLength: 100,
    strokeDasharray: `${portion} 100`, transform: "rotate(-90 32 32)" };
  return <div className="asb-resource-ring" data-resource-group={group}
    data-state={metric.state} data-available={available}>
    <svg className="asb-resource-ring-graphic" viewBox="0 0 64 64" aria-hidden="true" focusable="false">
      {partial && <defs>
        <pattern id={patternId} width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
          <rect width="3" height="6" fill="currentColor" />
        </pattern>
      </defs>}
      <circle className="asb-resource-ring-track" cx="32" cy="32" r="27" pathLength="100" />
      {available && portion > 0 && <g>
        <circle {...arc} className="asb-resource-ring-value" stroke="currentColor" />
        {partial && <circle {...arc} stroke={`url(#${patternId})`} />}
      </g>}
    </svg>
    <ResourceMetric metric={metric} kind={kind} />
  </div>;
}
