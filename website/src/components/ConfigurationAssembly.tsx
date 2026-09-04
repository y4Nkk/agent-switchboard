import { useId, useRef, useState, type KeyboardEvent } from "react";

import {
  configurationAssembly,
  type ConfigurationAssemblyClientId,
} from "../generated/configuration-assembly";
import { useSitePreferences } from "../use-site-preferences";

const clientIds = Object.keys(configurationAssembly) as ConfigurationAssemblyClientId[];

function nextTabIndex(current: number, key: string) {
  if (key === "Home") return 0;
  if (key === "End") return clientIds.length - 1;
  if (key === "ArrowRight" || key === "ArrowDown") return (current + 1) % clientIds.length;
  if (key === "ArrowLeft" || key === "ArrowUp") return (current - 1 + clientIds.length) % clientIds.length;
  return null;
}

export function ConfigurationAssembly() {
  const { content } = useSitePreferences();
  const [activeClient, setActiveClient] = useState<ConfigurationAssemblyClientId>("codex");
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const tabsId = useId();
  const active = configurationAssembly[activeClient];
  const copy = content.assembly.clients[activeClient];

  const selectTab = (index: number) => {
    const clientId = clientIds[index];
    setActiveClient(clientId);
    tabRefs.current[index]?.focus();
  };

  const onTabKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const nextIndex = nextTabIndex(index, event.key);
    if (nextIndex === null) return;
    event.preventDefault();
    selectTab(nextIndex);
  };

  return (
    <div className="configuration-assembly" aria-label={content.assembly.description}>
      <div className="assembly-tabs" role="tablist" aria-label={content.assembly.clientSelectorLabel}>
        {clientIds.map((clientId, index) => {
          const client = configurationAssembly[clientId];
          const selected = activeClient === clientId;
          return (
            <button
              key={clientId}
              ref={(element) => {
                tabRefs.current[index] = element;
              }}
              id={`${tabsId}-${clientId}-tab`}
              type="button"
              role="tab"
              tabIndex={selected ? 0 : -1}
              aria-selected={selected}
              aria-controls={`${tabsId}-${clientId}-panel`}
              className={`assembly-tab assembly-tab-${client.tone}`}
              onClick={() => setActiveClient(clientId)}
              onKeyDown={(event) => onTabKeyDown(event, index)}
            >
              {client.client}
            </button>
          );
        })}
      </div>

      <div
        id={`${tabsId}-${activeClient}-panel`}
        role="tabpanel"
        aria-labelledby={`${tabsId}-${activeClient}-tab`}
        className="assembly-stage"
      >
        <section className="assembly-block assembly-block-common">
          <h3>{copy.commonTitle}</h3>
          <dl>
            {active.commonFields.map((field) => (
              <div className="assembly-field" key={field.key}>
                <dt>{content.assembly.fieldLabels[field.key]}</dt>
                <dd>{field.value}</dd>
              </div>
            ))}
          </dl>
        </section>

        <span className="assembly-plus" aria-hidden="true">+</span>

        <section className={`assembly-block assembly-block-provider assembly-block-${active.tone}`}>
          <h3>{copy.providerTitle}</h3>
          <dl>
            {active.providerFields.map((field) => (
              <div className="assembly-field" key={field.key}>
                <dt>{content.assembly.fieldLabels[field.key]}</dt>
                <dd>{field.value}</dd>
              </div>
            ))}
          </dl>
        </section>

        <div className="assembly-arrow" aria-hidden="true">
          <svg viewBox="0 0 72 20" fill="none">
            <path d="M1 10h64M57 3l8 7-8 7" />
          </svg>
        </div>

        <section className="assembly-file" aria-label={`${content.assembly.combineLabel}：${active.fileName}`}>
          <div className="assembly-file-head">
            <strong>{active.fileName}</strong>
            <span>{copy.fileNote}</span>
          </div>
          <code className="assembly-path">{active.filePath}</code>
          <pre>
            <code>
              {active.codeLines.map((line, index) => (
                <span className="assembly-code-line" key={`${index}-${line}`}>
                  <span aria-hidden="true">{index + 1}</span>
                  {line}
                </span>
              ))}
            </code>
          </pre>
        </section>
      </div>
    </div>
  );
}
