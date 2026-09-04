import { useId } from "react";

import { useSitePreferences } from "../use-site-preferences";

/** A read-only rendering of the desktop switch executor's committed stages. */
export function WriteLifecycle() {
  const { content } = useSitePreferences();
  const { writeLifecycle } = content;
  const titleId = useId();

  return (
    <section className="transaction-replica" aria-labelledby={titleId}>
      <div className="transaction-replica-head">
        <h3 id={titleId}>{writeLifecycle.title}</h3>
      </div>
      <ol className="transaction-flow">
        {writeLifecycle.steps.map((step, index) => (
          <li className="transaction-flow-item" key={step}>
            <div className="transaction-step">
              <span className="transaction-step-dot" aria-hidden="true" />
              <span>{step}</span>
            </div>
            {index < writeLifecycle.steps.length - 1 && (
              <span className="transaction-connector" aria-hidden="true">
                →
              </span>
            )}
          </li>
        ))}
      </ol>
      <p className="transaction-recovery">
        <span aria-hidden="true" />
        {writeLifecycle.recovery}
      </p>
    </section>
  );
}
