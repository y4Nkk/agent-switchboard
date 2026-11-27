import { useLayoutEffect, useRef, type ReactNode, type RefObject } from "react";
import { ScrollArea } from "../ScrollArea";

export function ExtensionMarket({ label, active, hidden, controls, viewportRef, children }: {
  label: string;
  active: boolean;
  hidden?: boolean;
  controls?: ReactNode;
  viewportRef?: RefObject<HTMLDivElement | null>;
  children: ReactNode;
}) {
  const viewport = useRef<HTMLDivElement>(null);
  const readingPosition = useRef(0);
  const results = viewportRef ?? viewport;
  useLayoutEffect(() => {
    if (active && results.current) results.current.scrollTop = readingPosition.current;
  }, [active, results]);
  return (
    <div className="asb-ext-market" hidden={hidden}>
      {controls}
      <ScrollArea className="asb-ext-market-results" role="region" aria-label={label} tabIndex={0} viewportRef={results}
        onScroll={({ currentTarget }) => {
          if (active && currentTarget.clientHeight > 0) readingPosition.current = currentTarget.scrollTop;
        }}>
        <div className="asb-ext-market-content">{children}</div>
      </ScrollArea>
    </div>
  );
}

export function ExtensionMarketSearch({ label, onSubmit, children, actions }: {
  label: string;
  onSubmit: () => void;
  children: ReactNode;
  actions: ReactNode;
}) {
  return (
    <form className="asb-ext-market-form" aria-label={label} onSubmit={(event) => {
      event.preventDefault();
      onSubmit();
    }}>
      <div className="asb-ext-market-inputs">{children}</div>
      <div className="asb-ext-market-actions">{actions}</div>
    </form>
  );
}

export function ExtensionMarketResult({ title, origin, description, children, actions }: {
  title: string;
  origin: ReactNode;
  description?: string;
  children?: ReactNode;
  actions: ReactNode;
}) {
  return (
    <li className="asb-ext-market-result" aria-label={title}>
      <div className="asb-ext-market-result-copy">
        <div className="asb-ext-market-result-heading">
          <h3 className="asb-group-title">{title}</h3>
          {origin}
        </div>
        {description && <p className="asb-ext-market-description">{description}</p>}
        {children}
      </div>
      <div className="asb-ext-market-result-actions">{actions}</div>
    </li>
  );
}
