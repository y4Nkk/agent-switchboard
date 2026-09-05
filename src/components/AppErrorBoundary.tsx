import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  failed: boolean;
}

/** Keeps the desktop shell alive if one workspace surface throws during
    rendering. The title bar and independent tray window remain available. */
export class AppErrorBoundary extends Component<Props, State> {
  state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  componentDidCatch(_error: Error, _info: ErrorInfo) {
    // Deliberately avoid logging UI errors here: logs can contain rendered
    // config details. The visible fallback is the recovery surface.
  }

  render() {
    if (this.state.failed) {
      return (
        <section className="asb-panel asb-recovery-panel" role="alert" aria-label="界面恢复">
          <h2 className="asb-panel-title">界面未能加载</h2>
          <p className="asb-scope-note">可关闭窗口后从系统托盘重新打开应用。</p>
        </section>
      );
    }
    return this.props.children;
  }
}
