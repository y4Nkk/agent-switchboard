import { Component, type ReactNode } from "react";
import { useI18n } from "../i18n";
import { Button } from "./Button";

interface Props {
  active: boolean;
  children: ReactNode;
}

interface State {
  failed: boolean;
}

/** A healthy page keeps its subtree across navigation. Only a failed page
 * remounts, on explicit retry or its next inactive-to-active transition. */
export class PageErrorBoundary extends Component<Props, State> {
  state: State = { failed: false };

  componentDidUpdate(previous: Props) {
    if (this.state.failed && this.props.active && !previous.active) {
      this.setState({ failed: false });
    }
  }

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  render() {
    return this.state.failed
      ? <RecoveryMessage onRetry={() => this.setState({ failed: false })} />
      : this.props.children;
  }
}

function RecoveryMessage({ onRetry }: { onRetry: () => void }) {
  const { t } = useI18n();
  return (
    <section className="asb-panel asb-recovery-panel" role="alert" aria-label={t("recovery.aria")}>
      <h2 className="asb-recovery-title">{t("recovery.title")}</h2>
      <p className="asb-scope-note">{t("recovery.body")}</p>
      <Button variant="primary" onClick={onRetry}>{t("recovery.retry")}</Button>
    </section>
  );
}
