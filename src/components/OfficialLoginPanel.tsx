import { useMessageState } from "../i18n/use-message-state";
import { uiMessage } from "../i18n/errors";
import { useCallback, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  cancelOfficialLogin,
  pollOfficialLogin,
  startOfficialLogin,
  type AppKind,
  type OfficialLoginStatus,
} from "../api/client";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { CopyButton } from "./CopyButton";

const POLL_INTERVAL_MS = 3000;

type Phase = "idle" | "pending" | "completed" | "failed";

interface Props {
  app: AppKind;
  /** Reports the terminal result; the editor gates saving on a completion. */
  onFinished?: (completed: boolean) => void;
}

/** One client's official login flow: starts the backend session, walks the
 * user through the vendor page, and polls until the credentials land in the
 * client's native cache. It never renders credential material. */
export function OfficialLoginPanel({ app, onFinished }: Props) {
  const { t } = useI18n();
  const [phase, setPhase] = useState<Phase>("idle");
  const [userCode, setUserCode] = useState<string | null>(null);
  const [verificationUrl, setVerificationUrl] = useState("");
  const [message, setMessage] = useMessageState();
  const [starting, setStarting] = useState(false);
  const version = useRef(0);
  const inFlight = useRef(false);
  const sessionLive = useRef(false);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopPolling = useCallback(() => {
    if (timer.current !== null) {
      clearInterval(timer.current);
      timer.current = null;
    }
  }, []);

  // Leaving the flow mid-login abandons the backend session; without a live
  // session there is nothing to cancel.
  useEffect(
    () => () => {
      version.current += 1;
      stopPolling();
      if (sessionLive.current) {
        sessionLive.current = false;
        void cancelOfficialLogin(app).catch(() => undefined);
      }
    },
    [app, stopPolling],
  );

  const applyStatus = useCallback(
    (status: OfficialLoginStatus) => {
      setUserCode(status.userCode);
      setVerificationUrl(status.verificationUrl);
      if (status.phase === "pending") return;
      stopPolling();
      sessionLive.current = false;
      setPhase(status.phase);
      setMessage(status.message ?? (status.phase === "failed" ? uiMessage("clientConfig.login.failedFallback") : null));
      onFinished?.(status.phase === "completed");
    },
    [onFinished, setMessage, stopPolling],
  );

  const startPolling = useCallback(
    (current: number) => {
      stopPolling();
      timer.current = setInterval(() => {
        if (inFlight.current) return;
        inFlight.current = true;
        void pollOfficialLogin(app)
          .then((status) => {
            if (version.current === current) applyStatus(status);
          })
          .catch((caught) => {
            // One dropped poll must not kill a ten-minute login.
            if (version.current === current) setMessage(caught);
          })
          .finally(() => {
            inFlight.current = false;
          });
      }, POLL_INTERVAL_MS);
    },
    [app, applyStatus, stopPolling],
  );

  const start = async () => {
    if (starting) return;
    const current = version.current;
    setStarting(true);
    setMessage(null);
    let started = false;
    try {
      const login = await startOfficialLogin(app);
      started = true;
      sessionLive.current = true;
      if (version.current !== current) return;
      setUserCode(login.userCode);
      setVerificationUrl(login.verificationUrl);
      if (login.userCode === null) {
        // Claude: the authorize URL is the entry; open it right away.
        await openUrl(login.verificationUrl);
        if (version.current !== current) return;
      }
      setPhase("pending");
      startPolling(current);
    } catch (caught) {
      if (version.current !== current) return;
      if (started) {
        sessionLive.current = false;
        void cancelOfficialLogin(app).catch(() => undefined);
      }
      setPhase("failed");
      setMessage(caught);
      onFinished?.(false);
    } finally {
      if (version.current === current) setStarting(false);
    }
  };

  const cancel = () => {
    version.current += 1;
    stopPolling();
    sessionLive.current = false;
    setPhase("idle");
    setUserCode(null);
    setMessage(null);
    void cancelOfficialLogin(app).catch(() => undefined);
  };

  if (phase === "completed") {
    return (
      <div><p className="asb-provider-usage-state" role="status">{t("clientConfig.login.completed")}</p>
        {message && <p className="asb-warn-text" role="status">{message}</p>}</div>
    );
  }

  if (phase === "idle") {
    return (
      <Button
        variant="secondary"
        disabled={starting}
        onClick={() => void start()}
      >
        {starting ? t("clientConfig.login.starting") : t("clientConfig.login.start")}
      </Button>
    );
  }

  return (
    <div className="asb-official-login">
      {userCode ? (
        <div className="asb-official-login-code" role="status">
          <span>{t("clientConfig.login.codeLabel")}</span>
          <code>{userCode}</code>
          <CopyButton content={userCode} />
          <Button
            variant="secondary"
            onClick={() => { setMessage(null); void openUrl(verificationUrl).catch(setMessage); }}
          >
            {t("clientConfig.login.openVerification")}
          </Button>
        </div>
      ) : (
        <p className="asb-provider-usage-state" role="status">
          {t("clientConfig.login.browserOpened")}
        </p>
      )}
      <div className="asb-official-login-wait">
        <p className="asb-provider-usage-state" role="status">
          {t("clientConfig.login.waiting")}
        </p>
        <Button variant="secondary" onClick={cancel}>
          {t("clientConfig.login.cancel")}
        </Button>
      </div>
      {message && <p className="asb-warn-text" role="alert">{message}</p>}
    </div>
  );
}
