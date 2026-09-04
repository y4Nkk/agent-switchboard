import { chartTone } from "./chart-data";

interface ChartLegendItem {
  id: string;
  label: string;
  detail?: string;
}

interface Props {
  items: ChartLegendItem[];
}

/** Text stays paired with every tone so color never becomes the series name. */
export function ChartLegend({ items }: Props) {
  return (
    <ul className="asb-chart-legend" aria-label="图例">
      {items.map((item, index) => (
        <li key={item.id} className="asb-chart-legend-item" data-tone={chartTone(index)}>
          <span className="asb-chart-legend-swatch" aria-hidden="true" />
          <span className="asb-chart-legend-label" title={item.label}>{item.label}</span>
          {item.detail && <span className="asb-chart-legend-detail">{item.detail}</span>}
        </li>
      ))}
    </ul>
  );
}
