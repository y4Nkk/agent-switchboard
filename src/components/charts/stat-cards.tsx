/**
 * The usage summary band: one ledger strip whose values are peers in a single
 * real measurement, left-aligned on the view's shared baseline. Surface and
 * typography live in usage-workspace.css (.asb-usage-stat-band / -card).
 */

export type UsageStat = {
  label: string;
  value: string;
  /** Unit shown beside the value when the value alone is ambiguous. */
  unit?: string;
  detail?: string;
};

/** The model-usage summary band. */
export function StatCards({ stats }: { stats: UsageStat[] }) {
  return (
    <div className="asb-usage-stat-band">
      {stats.map((stat) => (
        <section key={stat.label} className="asb-usage-stat-card">
          <div className="asb-usage-stat-value-row">
            <p className="asb-usage-stat-value">{stat.value}</p>
            {stat.unit && <span className="asb-usage-stat-unit">{stat.unit}</span>}
          </div>
          <p className="asb-usage-stat-label">{stat.label}</p>
          {stat.detail && <p className="asb-usage-stat-detail">{stat.detail}</p>}
        </section>
      ))}
    </div>
  );
}
