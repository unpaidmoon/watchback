import { AlertCircle, CheckCircle2, LoaderCircle, RefreshCw, ScanLine, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { addAccountWithQr, cancelQrAccountLogin, getQrAccountLoginSnapshot, refreshQrAccountLogin } from "../../shared/api";
import type { QrAccountLoginProgressEvent, QrAccountLoginQrEvent, QrLoginViewState } from "../../shared/types";

type Props = {
  open: boolean;
  onCancel: () => void;
  onCompleted: (message: string | null) => Promise<void>;
  onUseExistingLogin: () => Promise<void>;
};

const text = {
  close: "\u5173\u95ed\u4e8c\u7ef4\u7801\u767b\u5f55",
  qrArea: "\u4e8c\u7ef4\u7801\u533a\u57df",
  qrAlt: "\u7f51\u6613\u5927\u795e\u767b\u5f55\u4e8c\u7ef4\u7801",
  qrHint: "\u4e8c\u7ef4\u7801\u4ec5\u7528\u4e8e\u672c\u6b21\u767b\u5f55",
  title: "\u4f7f\u7528\u7f51\u6613\u5927\u795e\u626b\u7801\u767b\u5f55",
  stepOne: "\u6253\u5f00\u7f51\u6613\u5927\u795e App",
  stepTwo: "\u626b\u63cf\u5de6\u4fa7\u4e8c\u7ef4\u7801",
  stepThree: "\u5728\u624b\u673a\u4e2d\u786e\u8ba4\u767b\u5f55",
  refresh: "\u5237\u65b0",
  cancel: "\u53d6\u6d88",
  other: "\u5176\u4ed6\u767b\u5f55\u65b9\u5f0f",
};

const cancellableStates = new Set<QrLoginViewState>(["starting", "loadingQr", "waitingForScan", "confirming", "expired", "failed"]);

function asViewState(value: string): QrLoginViewState {
  switch (value) {
    case "starting":
    case "openingLogin":
    case "selectingQrMode":
    case "acceptingPrivacy":
      return "starting";
    case "loadingQr":
    case "waitingForScan":
    case "confirming":
    case "capturing":
    case "saving":
    case "expired":
    case "ready":
    case "cancelling":
    case "failed":
      return value;
    case "cancelled":
    default:
      return "failed";
  }
}

function statusText(state: QrLoginViewState, message: string | null) {
  if (message) return message;
  return {
    starting: "\u6b63\u5728\u51c6\u5907\u5b89\u5168\u767b\u5f55\u73af\u5883\u2026",
    loadingQr: "\u6b63\u5728\u83b7\u53d6\u4e8c\u7ef4\u7801\u2026",
    waitingForScan: "\u7b49\u5f85\u626b\u7801",
    confirming: "\u5df2\u626b\u7801\uff0c\u8bf7\u5728\u624b\u673a\u4e2d\u786e\u8ba4",
    capturing: "\u767b\u5f55\u5df2\u786e\u8ba4\uff0c\u6b63\u5728\u8bc6\u522b\u8d26\u53f7\u2026",
    saving: "\u6b63\u5728\u5b89\u5168\u4fdd\u5b58\u8d26\u53f7\u767b\u5f55\u4fe1\u606f\u2026",
    expired: "\u4e8c\u7ef4\u7801\u5df2\u8fc7\u671f\uff0c\u8bf7\u5237\u65b0",
    ready: "\u8d26\u53f7\u6dfb\u52a0\u6210\u529f",
    cancelling: "\u6b63\u5728\u53d6\u6d88\u767b\u5f55\u2026",
    failed: "\u4e8c\u7ef4\u7801\u767b\u5f55\u6682\u4e0d\u53ef\u7528\uff0c\u8bf7\u5237\u65b0\u91cd\u8bd5\u6216\u4f7f\u7528\u5176\u4ed6\u767b\u5f55\u65b9\u5f0f",
  }[state];
}

function createPngUrl(base64: string) {
  const source = window.atob(base64);
  const bytes = Uint8Array.from(source, (char) => char.charCodeAt(0));
  return URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
}

function StatusIcon({ state }: { state: QrLoginViewState }) {
  if (state === "ready") return <CheckCircle2 aria-hidden="true" size={20} />;
  if (state === "expired" || state === "failed") return <AlertCircle aria-hidden="true" size={20} />;
  if (state === "waitingForScan" || state === "confirming") return <ScanLine aria-hidden="true" size={20} />;
  return <LoaderCircle className="qrAccountLoginSpinner" aria-hidden="true" size={20} />;
}

export function QrAccountLoginDialog({ open, onCancel, onCompleted, onUseExistingLogin }: Props) {
  const [state, setState] = useState<QrLoginViewState>("starting");
  const [message, setMessage] = useState<string | null>(null);
  const [qrUrl, setQrUrl] = useState<string | null>(null);
  const [actionPending, setActionPending] = useState(false);
  const rootRef = useRef<HTMLElement>(null);
  const urlRef = useRef<string | null>(null);
  const sessionIdRef = useRef<string | null>(null);
  const revisionRef = useRef(-1);
  const stateRef = useRef(state);
  const pendingRef = useRef(false);
  const completedRef = useRef(false);

  stateRef.current = state;
  pendingRef.current = actionPending;

  function clearImage() {
    if (urlRef.current) URL.revokeObjectURL(urlRef.current);
    urlRef.current = null;
    setQrUrl(null);
  }

  function cancelSession() {
    const sessionId = sessionIdRef.current;
    sessionIdRef.current = null;
    revisionRef.current = -1;
    clearImage();
    return sessionId ? cancelQrAccountLogin(sessionId) : Promise.resolve();
  }

  function acceptQr(payload: QrAccountLoginQrEvent) {
    if (payload.sessionId !== sessionIdRef.current || payload.revision <= revisionRef.current || payload.mimeType !== "image/png") return;
    try {
      const nextUrl = createPngUrl(payload.imageBase64);
      if (urlRef.current) URL.revokeObjectURL(urlRef.current);
      urlRef.current = nextUrl;
      revisionRef.current = payload.revision;
      setQrUrl(nextUrl);
      if (["starting", "loadingQr"].includes(stateRef.current)) {
        setState("waitingForScan");
        setMessage(null);
      }
    } catch {
      setState("failed");
      setMessage("\u4e8c\u7ef4\u7801\u6570\u636e\u65e0\u6548\uff0c\u8bf7\u5237\u65b0\u91cd\u8bd5\u6216\u4f7f\u7528\u5176\u4ed6\u767b\u5f55\u65b9\u5f0f\u3002");
    }
  }

  function acceptProgress(payload: QrAccountLoginProgressEvent) {
    if (payload.sessionId !== sessionIdRef.current) return;
    setState(asViewState(payload.phase));
    setMessage(payload.message);
  }

  function acceptResult(payload: { sessionId: string; result: unknown | null; message?: string | null; cancelled: boolean }) {
    if (payload.sessionId !== sessionIdRef.current || completedRef.current) return;
    if (!payload.result) {
      if (!payload.cancelled) {
        setState("failed");
        setMessage(payload.message ?? null);
      }
      return;
    }
    completedRef.current = true;
    setState("ready");
    setMessage(payload.message ?? null);
    window.setTimeout(() => void onCompleted(payload.message ?? null).finally(onCancel), 700);
  }

  useEffect(() => {
    if (!open) return;
    let disposed = false;
    completedRef.current = false;
    setState("starting");
    setMessage(null);
    clearImage();
    sessionIdRef.current = null;
    revisionRef.current = -1;
    let pollTimer: number | null = null;

    const poll = async (sessionId: string) => {
      const snapshot = await getQrAccountLoginSnapshot(sessionId);
      if (disposed || snapshot.sessionId !== sessionIdRef.current) return;
      acceptProgress({ sessionId, phase: snapshot.phase, message: snapshot.message });
      if (snapshot.imageBase64) {
        acceptQr({ sessionId, revision: snapshot.revision, mimeType: "image/png", imageBase64: snapshot.imageBase64 });
      }
      if (snapshot.completion) acceptResult(snapshot.completion);
    };

    const start = async () => {
      const { sessionId } = await addAccountWithQr();
      if (disposed) {
        void cancelQrAccountLogin(sessionId).catch(() => undefined);
        return;
      }
      sessionIdRef.current = sessionId;
      await poll(sessionId);
      pollTimer = window.setInterval(() => {
        void poll(sessionId).catch(() => undefined);
      }, 300);
    };
    const startTimer = window.setTimeout(() => {
      void start().catch(() => {
        if (!disposed) {
          setState("failed");
          setMessage("\u65e0\u6cd5\u542f\u52a8\u4e8c\u7ef4\u7801\u767b\u5f55\uff0c\u8bf7\u5237\u65b0\u91cd\u8bd5\u6216\u4f7f\u7528\u5176\u4ed6\u767b\u5f55\u65b9\u5f0f\u3002");
        }
      });
    }, 0);
    const watchdog = window.setTimeout(() => {
      if (!disposed && !sessionIdRef.current) {
        setState("failed");
        setMessage("\u767b\u5f55\u73af\u5883\u542f\u52a8\u8d85\u65f6\uff0c\u8bf7\u53d6\u6d88\u540e\u91cd\u8bd5\u6216\u4f7f\u7528\u5176\u4ed6\u767b\u5f55\u65b9\u5f0f\u3002");
      }
    }, 10_000);
    return () => {
      disposed = true;
      window.clearTimeout(startTimer);
      window.clearTimeout(watchdog);
      if (pollTimer !== null) window.clearInterval(pollTimer);
      void cancelSession().catch(() => undefined);
    };
  // One isolated QR-login backend session is created for every dialog opening.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    rootRef.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && cancellableStates.has(stateRef.current) && !pendingRef.current) {
        event.preventDefault();
        void requestCancel();
      }
      if (event.key === "Tab") {
        const buttons = Array.from(rootRef.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
        if (!buttons.length) return;
        const current = buttons.indexOf(document.activeElement as HTMLButtonElement);
        const next = event.shiftKey ? (current <= 0 ? buttons.length - 1 : current - 1) : (current + 1) % buttons.length;
        event.preventDefault();
        buttons[next]?.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      if (previousFocus?.isConnected) previousFocus.focus();
    };
  // requestCancel is only called after the dialog has mounted.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  async function requestCancel() {
    if (actionPending || !cancellableStates.has(state)) return;
    setActionPending(true);
    setState("cancelling");
    try {
      await cancelSession();
      onCancel();
    } catch {
      setState("failed");
      setMessage("\u53d6\u6d88\u767b\u5f55\u5931\u8d25\uff0c\u8bf7\u91cd\u8bd5\u3002");
    } finally {
      setActionPending(false);
    }
  }

  async function requestRefresh() {
    const sessionId = sessionIdRef.current;
    if (!sessionId || actionPending || !["waitingForScan", "expired", "failed"].includes(state)) return;
    setActionPending(true);
    setState("loadingQr");
    setMessage(null);
    clearImage();
    try {
      await refreshQrAccountLogin(sessionId);
    } catch {
      setState("failed");
      setMessage("\u5237\u65b0\u4e8c\u7ef4\u7801\u5931\u8d25\uff0c\u8bf7\u91cd\u8bd5\u6216\u4f7f\u7528\u5176\u4ed6\u767b\u5f55\u65b9\u5f0f\u3002");
    } finally {
      setActionPending(false);
    }
  }

  async function requestExistingLogin() {
    if (actionPending || !cancellableStates.has(state)) return;
    setActionPending(true);
    setState("cancelling");
    try {
      await cancelSession();
      onCancel();
      await onUseExistingLogin();
    } catch {
      setState("failed");
      setMessage("\u65e0\u6cd5\u5207\u6362\u5230\u5176\u4ed6\u767b\u5f55\u65b9\u5f0f\uff0c\u8bf7\u91cd\u8bd5\u3002");
    } finally {
      setActionPending(false);
    }
  }

  if (!open) return null;
  const canRefresh = !actionPending && ["waitingForScan", "expired", "failed"].includes(state);
  const canCancel = !actionPending && cancellableStates.has(state);
  const canOther = !actionPending && cancellableStates.has(state);
  const failure = state === "failed" ? statusText(state, message) : null;

  return <div className="qrAccountLoginBackdrop" role="presentation" onMouseDown={canCancel ? () => void requestCancel() : undefined}>
    <section ref={rootRef} className="qrAccountLoginDialog" role="dialog" aria-modal="true" aria-labelledby="qr-account-login-title" aria-describedby="qr-account-login-description qr-account-login-status" onMouseDown={(event) => event.stopPropagation()}>
      <button className="iconButton qrAccountLoginClose" type="button" aria-label={text.close} onClick={() => void requestCancel()} disabled={!canCancel}><X aria-hidden="true" size={20} /></button>
      <div className="qrAccountLoginQrPane" aria-label={text.qrArea}>
        <div className={`qrAccountLoginQrStage qrAccountLoginQrStage--${state}`}>
          {qrUrl && state !== "ready" ? <img src={qrUrl} alt={text.qrAlt} width={256} height={256} /> : state === "ready" ? <CheckCircle2 aria-hidden="true" size={52} /> : <div className="qrAccountLoginQrPlaceholder" aria-hidden="true">{state === "failed" || state === "expired" ? <AlertCircle size={40} /> : <LoaderCircle className="qrAccountLoginSpinner" size={40} />}</div>}
        </div>
        <p>{text.qrHint}</p>
      </div>
      <div className="qrAccountLoginContent">
        <div className="qrAccountLoginCopy"><h3 id="qr-account-login-title">{text.title}</h3><ol id="qr-account-login-description"><li>{text.stepOne}</li><li>{text.stepTwo}</li><li>{text.stepThree}</li></ol><div id="qr-account-login-status" className={`qrAccountLoginStatus qrAccountLoginStatus--${state}`} aria-live="polite"><StatusIcon state={state} /><span>{statusText(state, message)}</span></div>{failure ? <p className="qrAccountLoginError" role="alert">{failure}</p> : null}</div>
        <div className="qrAccountLoginActions"><button className="ghostButton" type="button" onClick={() => void requestRefresh()} disabled={!canRefresh}><RefreshCw aria-hidden="true" size={18} />{text.refresh}</button><button className="ghostButton" type="button" onClick={() => void requestCancel()} disabled={!canCancel}>{text.cancel}</button><button className="primaryButton" type="button" onClick={() => void requestExistingLogin()} disabled={!canOther}>{text.other}</button></div>
      </div>
    </section>
  </div>;
}
