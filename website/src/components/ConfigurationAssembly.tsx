import {
  useId,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
} from "react";

import {
  configurationAssembly,
  type ConfigurationAssemblyCommonField,
  type ConfigurationAssemblyControlValue,
  type ConfigurationAssemblyClientId,
} from "../generated/configuration-assembly";
import { useSitePreferences } from "../use-site-preferences";

const clientIds = Object.keys(configurationAssembly) as ConfigurationAssemblyClientId[];
type CommonAssemblyField = {
  key: ConfigurationAssemblyCommonField["key"];
  value: ConfigurationAssemblyControlValue;
  control: ConfigurationAssemblyCommonField["control"];
  options: readonly ConfigurationAssemblyControlValue[];
};
type AssemblySliderStyle = CSSProperties & { "--assembly-slider-position": string };

function nextTabIndex(current: number, key: string) {
  if (key === "Home") return 0;
  if (key === "End") return clientIds.length - 1;
  if (key === "ArrowRight" || key === "ArrowDown") return (current + 1) % clientIds.length;
  if (key === "ArrowLeft" || key === "ArrowUp") return (current - 1 + clientIds.length) % clientIds.length;
  return null;
}

function CommonSettingControl({
  field,
  labels,
}: {
  field: CommonAssemblyField;
  labels: Record<ConfigurationAssemblyControlValue, string>;
}) {
  const selectedIndex = field.options.indexOf(field.value);
  const position = field.options.length > 1 ? (selectedIndex / (field.options.length - 1)) * 100 : 0;

  if (field.control === "slider") {
    const style = { "--assembly-slider-position": `${position}%` } as AssemblySliderStyle;
    return (
      <span className="assembly-control assembly-control-slider" aria-hidden="true" style={style}>
        <span className="assembly-slider-track">
          <span className="assembly-slider-fill" />
          <span className="assembly-slider-thumb" />
          <span className="assembly-slider-ticks">
            {field.options.map((option) => (
              <span key={option} data-selected={option === field.value || undefined} />
            ))}
          </span>
        </span>
        <span className="assembly-slider-values">
          {field.options.map((option) => (
            <span key={option} data-selected={option === field.value || undefined}>
              {labels[option]}
            </span>
          ))}
        </span>
      </span>
    );
  }

  return (
    <span className="assembly-control assembly-control-segments" aria-hidden="true">
      {field.options.map((option) => (
        <span className="assembly-control-option" key={option} data-selected={option === field.value || undefined}>
          {labels[option]}
        </span>
      ))}
    </span>
  );
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
              className="assembly-tab"
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
          <div className="assembly-common-settings">
            {active.commonFields.map((field) => (
              <div className="assembly-common-setting" key={field.key}>
                <span className="assembly-common-setting-label">
                  {content.assembly.fieldLabels[field.key]}
                </span>
                <CommonSettingControl field={field} labels={content.assembly.controlLabels} />
              </div>
            ))}
          </div>
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
