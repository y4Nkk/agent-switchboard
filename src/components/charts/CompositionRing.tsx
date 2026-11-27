interface CompositionPart {
  id: string;
  value: number;
  color: string;
}

export function CompositionRing({ parts }: { parts: readonly CompositionPart[] }) {
  const visible = parts.filter(({ value }) => value > 0);
  const total = visible.reduce((sum, { value }) => sum + value, 0);
  const gap = visible.length > 1 ? Math.min(2.25, 40 / visible.length) : 0;
  const available = 100 - visible.length * gap;
  let offset = gap / 2;
  return <svg viewBox="0 0 100 100" fill="none" strokeWidth={10} aria-hidden="true" focusable="false">
    {total === 0 && <circle cx="50" cy="50" r="40" stroke="var(--asb-hairline)" />}
    {visible.map(({ id, value, color }) => {
      const length = value / total * available;
      const start = offset;
      offset += length + gap;
      return <circle key={id} cx="50" cy="50" r="40" pathLength="100" transform="rotate(-90 50 50)"
        stroke={color} strokeDasharray={`${length} ${100 - length}`} strokeDashoffset={-start} />;
    })}
  </svg>;
}
