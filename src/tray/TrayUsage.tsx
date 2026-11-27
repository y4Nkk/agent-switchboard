import { useLayoutEffect, useRef, useState } from "react";
import { TriangleAlert } from "lucide-react";
import type { TrayReading, TrayUsageContent } from "./tray-usage";

function Reading({ reading }: { reading: TrayReading }) {
  return <span className="tray-usage-reading" data-tone={reading.tone}>
    {reading.name && <span className="tray-usage-name">{reading.name}</span>}
    <span className="tray-usage-value">{reading.value}</span>
  </span>;
}

export function TrayUsage({ content }: { content: TrayUsageContent }) {
  const { readings, alert } = content;
  const host = useRef<HTMLSpanElement>(null);
  const measure = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(2);
  const total = readings.length;
  useLayoutEffect(() => {
    const element = host.current;
    const ruler = measure.current;
    if (!element || !ruler) return;
    const resize = () => {
      const available = element.clientWidth;
      const gap = parseFloat(getComputedStyle(element).columnGap) || 0;
      const children = Array.from(ruler.children) as HTMLElement[];
      const fixed = children.find((child) => child.dataset.measure === "fixed");
      const more = children.find((child) => child.dataset.measure === "more");
      const values = children.filter((child) => child.dataset.measure === "reading");
      let width = fixed?.getBoundingClientRect().width ?? 0;
      let count = 0;
      for (const child of values) {
        const nextWidth = width + child.getBoundingClientRect().width + (count > 0 || fixed ? gap : 0);
        const overflowWidth = count + 1 < content.readings.length ? (more?.getBoundingClientRect().width ?? 0) + gap : 0;
        if (nextWidth + overflowWidth > available) break;
        width = nextWidth;
        count += 1;
      }
      setVisible(count);
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(element);
    observer.observe(ruler);
    for (const child of ruler.children) observer.observe(child);
    return () => observer.disconnect();
  }, [content]);
  if (readings.length === 0 && !alert) return null;
  return <span ref={host} className="tray-provider-balance">
    {readings.slice(0, visible).map((reading, index) => <Reading key={index} reading={reading} />)}
    {total > visible && <span className="tray-usage-more">+{total - visible}</span>}
    {alert && <span className="tray-usage-alert" role="img" aria-label={alert.title}>
      <TriangleAlert size={14} aria-hidden="true" /></span>}
    <span ref={measure} className="tray-usage-measure" aria-hidden="true">
      {readings.slice(0, 2).map((reading, index) => <span key={index} data-measure="reading"><Reading reading={reading} /></span>)}
      <span data-measure="more">+{total}</span>
      {alert && <span data-measure="fixed"><TriangleAlert size={14} aria-hidden="true" /></span>}
    </span>
  </span>;
}
