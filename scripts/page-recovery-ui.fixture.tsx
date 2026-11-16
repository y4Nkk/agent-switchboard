import { useState } from "react";
import { createRoot } from "react-dom/client";
import { PageErrorBoundary } from "../src/components/PageErrorBoundary";
import { ProviderDiagnosticDetails } from "../src/components/ProviderDiagnosticDetails";
import { applyLanguagePreference } from "../src/i18n/current";
import { providerDiagnostic } from "../tests/fixtures/provider-diagnostic";

applyLanguagePreference("en-US");

function Draft() {
  const [value, setValue] = useState("");
  return <input aria-label="Healthy draft" value={value} onChange={(event) => setValue(event.target.value)} />;
}

function Fault({ fail, name }: { fail: boolean; name: "page" | "tab" }) {
  if (fail) throw new Error("isolated-page-render-fixture");
  return <p>Recovered {name} content</p>;
}

function Fixture() {
  const [page, setPage] = useState("healthy");
  const [fail, setFail] = useState(false);
  const [retryAfter, setRetryAfter] = useState<string | null>("120");
  const [nested, setNested] = useState("healthy");
  return <>
    <nav aria-label="Fixture navigation">
      <button onClick={() => setPage("healthy")}>Healthy page</button>
      <button onClick={() => setPage("fault")}>Fault page</button>
      <button onClick={() => setFail((previous) => !previous)}>Toggle failure</button>
      <button onClick={() => applyLanguagePreference("zh-CN")}>Chinese</button>
      <button onClick={() => applyLanguagePreference("en-US")}>English</button>
    </nav>
    <section hidden={page !== "healthy"}>
      <PageErrorBoundary active={page === "healthy"}><Draft /></PageErrorBoundary>
    </section>
    <section hidden={page !== "fault"}>
      <PageErrorBoundary active={page === "fault"}><Fault fail={fail} name="page" /></PageErrorBoundary>
    </section>
    <nav aria-label="Nested navigation">
      <button onClick={() => setNested("healthy")}>Healthy tab</button>
      <button onClick={() => setNested("fault")}>Fault tab</button>
    </nav>
    <section hidden={nested !== "healthy"}>
      <PageErrorBoundary active={nested === "healthy"}><input aria-label="Healthy tab draft" /></PageErrorBoundary>
    </section>
    <section hidden={nested !== "fault"}>
      <PageErrorBoundary active={nested === "fault"}><Fault fail={fail} name="tab" /></PageErrorBoundary>
    </section>
    <section aria-label="Diagnostic fixture">
      <select aria-label="Retry-After fixture" value={retryAfter ?? "absent"}
        onChange={(event) => setRetryAfter(event.target.value === "absent" ? null : event.target.value)}>
        <option value="120">120</option><option value="0">0</option>
        <option value="Wed, 21 Oct 2026 07:28:00 GMT">HTTP date</option><option value="absent">Absent</option>
      </select>
      <ProviderDiagnosticDetails diagnostic={{ ...providerDiagnostic, retryAfter }} />
    </section>
  </>;
}

createRoot(document.getElementById("root")!).render(<Fixture />);
