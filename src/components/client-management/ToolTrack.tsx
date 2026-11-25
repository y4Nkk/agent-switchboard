import type { CSSProperties, ReactNode } from "react";
import { FactList } from "../FactList";

/** `known` / `unknown` are readings; the rest are the progress of a step. */
export type ToolStationState = "known" | "unknown" | "done" | "current" | "pending" | "failed" | "attention";

export interface ToolStation {
  label: ReactNode;
  value?: ReactNode;
  state?: ToolStationState;
  /** The segment from this station to the next one. */
  link?: "same" | "upgrade" | "await" | "done";
}

interface Props {
  stations: ToolStation[];
  /** A rail of readings: the first station is where the reading is taken. */
  reading?: boolean;
  /** Sets the first reading in the instrument numeral. */
  emphasis?: boolean;
  /** A rail of labels without readings: it ends at its last station instead
   * of stretching across the surface it sits in. */
  fit?: boolean;
}

/** Stations on one rail: the version readings of an installation or the
 * phases of an update. Every station is a FactList pair (label above the
 * value); the dot and the line between two stations only restate what the
 * station texts already say, so they stay out of the accessibility tree. */
export function ToolTrack({ stations, reading = false, emphasis = false, fit = false }: Props) {
  return (
    <div className="asb-tool-rail" data-reading={reading || undefined} data-emphasis={emphasis || undefined}
      data-fit={fit || undefined}
      style={{ "--asb-track-stations": stations.length } as CSSProperties}>
      <FactList inline rows={stations.map((station) => ({
        label: <><i className="asb-tool-dot" aria-hidden="true" data-state={station.state ?? "known"}
          data-link={station.link} />{station.label}</>,
        value: station.value ?? null,
      }))} />
    </div>
  );
}
