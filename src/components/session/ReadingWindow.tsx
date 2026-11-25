import { useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode, type RefObject } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";

export interface ReadingPosition {
  index: number;
  inset: number;
  height?: number;
}

interface Props {
  count: number;
  estimate: number;
  position?: RefObject<ReadingPosition>;
  jump?: ReadingJump | null;
  children: (index: number) => ReactNode;
}

interface ReadingJump {
  index: number;
  afterContent?: "start" | "match";
}

function blockAt(count: number, offsetAt: (index: number) => number, position: number): number {
  let start = 0;
  let end = count;
  while (end - start > 1) {
    const middle = Math.floor((start + end) / 2);
    if (offsetAt(middle) <= position) start = middle;
    else end = middle;
  }
  return start;
}

function renderedOffsets(root: HTMLElement, offsets: Float64Array) {
  const corrections = Array.from(root.querySelectorAll<HTMLElement>(":scope > [data-reading-block]"), (block) => {
    const index = Number(block.dataset.readingBlock);
    const height = Math.max(1, block.getBoundingClientRect().height);
    return { index, delta: height - (offsets[index + 1] - offsets[index]) };
  });
  return (index: number) => offsets[index] + corrections.reduce((delta, block) =>
    block.index < index ? delta + block.delta : delta, 0);
}

function seekLoadedBlock(root: HTMLElement, view: HTMLElement, index: number, mode: "start" | "match") {
  const block = root.querySelector<HTMLElement>(`:scope > [data-reading-block="${index}"]`);
  if (block?.firstElementChild?.getAttribute("aria-busy") !== "false") return false;
  const mark = mode === "match" ? block.querySelector("mark")?.getClientRects()[0] : undefined;
  const top = (mark ?? block.getBoundingClientRect()).top;
  view.scrollTop += top - view.getBoundingClientRect().top - (mark ? view.clientHeight / 2 : 0);
  return true;
}

function useWindowRange({ count, position, jump }: Props, offsets: Float64Array) {
  const host = useRef<HTMLDivElement>(null);
  const initial = Math.min(count - 1, Math.max(0, jump?.index ?? position?.current.index ?? 0));
  const [range, setRange] = useState({ start: Math.max(0, initial), end: initial + 1 });
  const applied = useRef<ReadingJump | null>(null);
  const initialized = useRef(false);
  useLayoutEffect(() => {
    const root = host.current;
    const view = root?.closest<HTMLElement>(".asb-session-transcript > .asb-scroll-viewport");
    if (!view || !root || count === 0) return;
    let frame = 0;
    const relativeTop = () => view.getBoundingClientRect().top - root.getBoundingClientRect().top;
    const sync = () => {
      if (jump?.afterContent && jump !== applied.current) {
        if (!seekLoadedBlock(root, view, Math.min(count - 1, jump.index), jump.afterContent)) return;
        applied.current = jump;
        content?.disconnect();
      }
      const top = relativeTop();
      const height = view.clientHeight;
      const bottom = top + height;
      const offsetAt = renderedOffsets(root, offsets);
      const total = offsetAt(count);
      const start = blockAt(count, offsetAt, top - height);
      const end = bottom + height <= 0 || top - height >= total
        ? start : blockAt(count, offsetAt, bottom + height) + 1;
      setRange((previous) => previous.start === start && previous.end === end ? previous : { start, end });
      if (position && height > 0) {
        const index = blockAt(count, offsetAt, top);
        position.current = { index, inset: Math.max(0, top - offsetAt(index)),
          height: offsetAt(index + 1) - offsetAt(index) };
      }
    };
    const schedule = () => {
      if (frame) return;
      frame = window.requestAnimationFrame(() => { frame = 0; sync(); });
    };
    const content = jump?.afterContent && jump !== applied.current ? new MutationObserver(schedule) : null;
    content?.observe(root, { subtree: true, attributes: true, attributeFilter: ["aria-busy"], childList: true });
    if (!jump?.afterContent && (!initialized.current || (jump && jump !== applied.current))) {
      const index = Math.min(count - 1, Math.max(0, jump?.index ?? position?.current.index ?? 0));
      const inset = jump ? 0 : Math.max(0, Math.min(position?.current.inset ?? 0, offsets[index + 1] - offsets[index] - 1));
      if (jump || position) view.scrollTop += offsets[index] + inset - relativeTop();
      applied.current = jump ?? null;
      initialized.current = true;
    }
    if (content) schedule(); else sync();
    view.addEventListener("scroll", schedule, { passive: true });
    const size = new ResizeObserver(schedule);
    size.observe(view);
    size.observe(root);
    return () => {
      view.removeEventListener("scroll", schedule);
      size.disconnect();
      content?.disconnect();
      if (frame) window.cancelAnimationFrame(frame);
    };
  }, [count, offsets, position, jump]);
  const pinned = jump?.afterContent && jump !== applied.current ? Math.min(count - 1, jump.index) : null;
  return { host, range: pinned === null ? range : { start: pinned, end: pinned + 1 } };
}

export function ReadingPlaceholder({ error, retry }: { error: string | null; retry: () => void }) {
  const { t } = useI18n();
  return <div className="asb-reading-placeholder">
    {error ? <div>
      <p className="asb-warn-text" role="alert">{error}</p>
      <Button variant="secondary" onClick={retry}>{t("sessions.transcript.retry")}</Button>
    </div> : <p role="status">{t("sessions.transcript.loading.aria")}</p>}
  </div>;
}

/** Both message batches and long text share the reader's native scroll viewport. */
export function ReadingWindow(props: Props) {
  const { count, estimate, children } = props;
  const [heights, setHeights] = useState<ReadonlyMap<number, number>>(() => {
    const restored = props.position?.current;
    return restored?.height ? new Map([[restored.index, restored.height]]) : new Map();
  });
  const offsets = useMemo(() => {
    const offsets = new Float64Array(count + 1);
    for (let index = 0; index < count; index += 1) {
      offsets[index + 1] = offsets[index] + (heights.get(index) ?? estimate);
    }
    return offsets;
  }, [count, estimate, heights]);
  const { host, range } = useWindowRange(props, offsets);
  const start = Math.min(count, range.start);
  const end = Math.min(count, range.end);
  useLayoutEffect(() => {
    const observer = new ResizeObserver((entries) => {
      const measurements = entries.map(({ target, borderBoxSize, contentRect }) => ({
        index: Number((target as HTMLDivElement).dataset.readingBlock),
        height: Math.max(1, borderBoxSize[0]?.blockSize ?? contentRect.height),
      }));
      setHeights((current) => {
        let next: Map<number, number> | undefined;
        for (const { index, height } of measurements) {
          if (height !== current.get(index)) {
            next ??= new Map(current);
            next.set(index, height);
          }
        }
        return next ?? current;
      });
    });
    for (const block of host.current?.querySelectorAll(":scope > [data-reading-block]") ?? []) observer.observe(block);
    return () => observer.disconnect();
  }, [host, start, end]);

  return <div ref={host} className="asb-reading-window">
    <div aria-hidden="true" style={{ height: offsets[start] }} />
    {Array.from({ length: end - start }, (_, index) => start + index).map((index) =>
      <div key={index} className="asb-reading-block" data-reading-block={index}
        style={{ "--asb-reading-estimate": `${heights.get(index) ?? estimate}px` } as CSSProperties}>
        {children(index)}
      </div>)}
    <div aria-hidden="true" style={{ height: offsets[count] - offsets[end] }} />
  </div>;
}
