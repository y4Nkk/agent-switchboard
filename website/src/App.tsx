import { useEffect } from "react";

import { ConfigurationAssembly } from "./components/ConfigurationAssembly";
import { FinalCta } from "./components/FinalCta";
import { Hero } from "./components/Hero";
import { SiteHeader } from "./components/SiteHeader";
import { SitePreferencesProvider } from "./site-preferences";
import { useSitePreferences } from "./use-site-preferences";

function useReveal() {
  useEffect(() => {
    const elements = Array.from(document.querySelectorAll<HTMLElement>(".reveal"));
    if (!("IntersectionObserver" in window)) {
      for (const element of elements) element.classList.add("is-visible");
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            entry.target.classList.add("is-visible");
            observer.unobserve(entry.target);
          }
        }
      },
      { threshold: 0.15 },
    );
    for (const element of elements) observer.observe(element);
    return () => observer.disconnect();
  }, []);
}

function PreviewReplica() {
  const { content } = useSitePreferences();
  return (
    <div className="preview-replica" role="img" aria-label={content.preview.description}>
      <div className="preview-replica-head">
        <h3>{content.preview.title}</h3>
        <div className="preview-replica-actions" aria-hidden="true">
          <span className="preview-chip preview-chip-ghost">{content.preview.cancelLabel}</span>
          <span className="preview-chip preview-chip-primary">{content.preview.confirmLabel}</span>
        </div>
      </div>
      <div className="preview-rows">
        {content.preview.changes.map((change) => (
          <div className="preview-row" key={change.key}>
            <span className="preview-key">{change.key}</span>
            <span className="preview-change">
              <span className="preview-val preview-val-old">{change.from}</span>
              <span className="preview-arrow" aria-hidden="true">
                →
              </span>
              <span className="preview-val preview-val-new">{change.to}</span>
            </span>
          </div>
        ))}
      </div>
      <div className="preview-file">
        <span>{content.preview.file}</span>
        <span>{content.preview.fileNote}</span>
      </div>
      <div className="preview-code" aria-hidden="true">
        {content.preview.codeLines.map((line, index) => (
          <span className="preview-code-line" key={index}>
            <span className="preview-code-no">{index + 1}</span>
            <code>{line}</code>
          </span>
        ))}
      </div>
    </div>
  );
}

function SitePage() {
  const { content } = useSitePreferences();
  useReveal();
  return (
    <>
      <SiteHeader />
      <main>
        <Hero />
        <section id="assembly" className="site-section site-section-assembly">
          <div className="site-container">
            <div className="section-head reveal">
              <h2>{content.assembly.title}</h2>
              <p>{content.assembly.description}</p>
            </div>
            <div className="reveal">
              <ConfigurationAssembly />
            </div>
          </div>
        </section>
        <section id="preview" className="site-section">
          <div className="site-container">
            <div className="section-head reveal">
              <h2>{content.preview.title}</h2>
              <p>{content.preview.description}</p>
            </div>
            <div className="reveal">
              <PreviewReplica />
            </div>
          </div>
        </section>
        <FinalCta />
      </main>
    </>
  );
}

export default function App() {
  return (
    <SitePreferencesProvider>
      <SitePage />
    </SitePreferencesProvider>
  );
}
