import { AlertCircle, CheckCircle2, LoaderCircle, RefreshCw, ScanLine, X } from "lucide-react";
import { useEffect, useRef, useState, type RefObject } from "react";
import { addAccountWithQr, cancelQrAccountLogin, getAccountOperationStatus, getActiveQrAccountLogin, getQrAccountLoginSnapshot, refreshQrAccountLogin } from "../../shared/api";
import type { QrAccountLoginSnapshot, QrLoginViewState } from "../../shared/types";
import { createQrAccountLoginController, createQrLoginRequestId } from "./useQrAccountLogin";
import type { QrAccountLoginControllerState } from "./useQrAccountLogin";

type Props = { open: boolean; onCancel: () => void; onCompleted: (message: string | null) => Promise<void>; onUseExistingLogin: () => Promise<void> };
const cancellableStates = new Set<QrLoginViewState>(["starting", "loadingQr", "waitingForScan", "confirming", "capturing", "expired", "failed"]);
const text = { close: "关闭二维码登录", qrArea: "二维码区域", qrAlt: "网易大神登录二维码", qrHint: "二维码仅用于本次登录", title: "使用网易大神扫码登录", stepOne: "打开网易大神 App", stepTwo: "扫描左侧二维码", stepThree: "在手机中确认登录", refresh: "刷新", cancel: "取消", other: "其他登录方式" };
const imageStates = new Set<QrLoginViewState>(["waitingForScan", "confirming", "capturing"]);

function asViewState(value: string): QrLoginViewState {
  if (["openingLogin", "selectingQrMode", "acceptingPrivacy"].includes(value)) return "starting";
  if (value === "recovering" || value === "cancelled") return "failed";
  return value as QrLoginViewState;
}

function statusText(state: QrLoginViewState, message: string | null) {
  if (message) return message;
  return { starting: "正在准备安全登录环境…", loadingQr: "正在获取二维码…", waitingForScan: "等待扫码", confirming: "已扫码，请在手机中确认", capturing: "登录已确认，正在识别账号…", saving: "正在安全保存账号登录信息…", expired: "二维码已过期，请刷新", ready: "账号添加成功", cancelling: "正在取消登录…", failed: "二维码登录暂不可用，请刷新重试或使用其他登录方式" }[state];
}

function StatusIcon({ state }: { state: QrLoginViewState }) {
  if (state === "ready") return <CheckCircle2 aria-hidden="true" size={20} />;
  if (state === "expired" || state === "failed") return <AlertCircle aria-hidden="true" size={20} />;
  if (state === "waitingForScan" || state === "confirming") return <ScanLine aria-hidden="true" size={20} />;
  return <LoaderCircle className="qrAccountLoginSpinner" aria-hidden="true" size={20} />;
}

export type QrAccountLoginDialogViewProps = {
  current: QrAccountLoginControllerState;
  imageUrl?: string | null;
  rootRef: RefObject<HTMLElement>;
  onDismiss: () => void;
  onRefresh: () => void;
  onUseExistingLogin: () => void;
  onEndAndRetry: () => void;
};

type QrObjectUrlApi = {
  decode: (base64: string) => Uint8Array;
  createObjectUrl: (blob: Blob) => string;
  revokeObjectUrl: (url: string) => void;
};

export function createQrImageObjectUrl(base64: string, api?: QrObjectUrlApi) {
  const runtime = api ?? {
    decode: (value: string) => Uint8Array.from(atob(value), (character) => character.charCodeAt(0)),
    createObjectUrl: (blob: Blob) => URL.createObjectURL(blob),
    revokeObjectUrl: (url: string) => URL.revokeObjectURL(url),
  };
  const decoded = runtime.decode(base64);
  const bytes = new Uint8Array(decoded.byteLength);
  bytes.set(decoded);
  const url = runtime.createObjectUrl(new Blob([bytes.buffer], { type: "image/png" }));
  return { url, revoke: () => runtime.revokeObjectUrl(url) };
}

function useQrImageUrl(imageBase64: string | null, visible: boolean) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!imageBase64 || !visible) {
      setUrl(null);
      return;
    }
    const objectUrl = createQrImageObjectUrl(imageBase64);
    setUrl(objectUrl.url);
    return objectUrl.revoke;
  }, [imageBase64, visible]);
  return url;
}

export function handleQrDialogKeyDown(
  event: Pick<KeyboardEvent, "key" | "preventDefault"> & Partial<Pick<KeyboardEvent, "shiftKey">>,
  canDismiss: boolean,
  onDismiss: () => void,
  focusable: readonly Pick<HTMLElement, "focus">[] = [],
  activeElement: Pick<HTMLElement, "focus"> | null = null,
) {
  if (event.key === "Escape" && canDismiss) {
    event.preventDefault();
    onDismiss();
    return;
  }
  if (event.key !== "Tab" || focusable.length === 0) return;
  const current = activeElement ? focusable.indexOf(activeElement) : -1;
  const next = event.shiftKey
    ? (current <= 0 ? focusable.length - 1 : current - 1)
    : (current + 1) % focusable.length;
  event.preventDefault();
  focusable[next]?.focus();
}

export function restoreQrDialogFocus(previous: Pick<HTMLElement, "isConnected" | "focus"> | null) {
  if (previous?.isConnected) previous.focus();
}

export async function completeQrAccountLogin(
  completion: NonNullable<QrAccountLoginSnapshot["completion"]>,
  onCompleted: (message: string | null) => Promise<void>,
  onClose: () => void,
) {
  try {
    await onCompleted(completion.result?.message ?? completion.message ?? null);
  } finally {
    onClose();
  }
}

export function QrAccountLoginDialogView({
  current,
  imageUrl = null,
  rootRef,
  onDismiss,
  onRefresh,
  onUseExistingLogin,
  onEndAndRetry,
}: QrAccountLoginDialogViewProps) {
  const state = asViewState(current.phase);
  const canRefresh = !current.actionPending && ["waitingForScan", "expired", "failed"].includes(state);
  const canCancel = !current.actionPending && cancellableStates.has(state);
  const qrUrl = current.imageBase64 && imageStates.has(state) ? imageUrl : null;
  const failure = current.recoveryMessage ?? (state === "failed" ? statusText(state, current.message) : null);

  return <div className="qrAccountLoginBackdrop" role="presentation" onMouseDown={canCancel ? onDismiss : undefined}>
    <section ref={rootRef} className="qrAccountLoginDialog" role="dialog" aria-modal="true" aria-labelledby="qr-account-login-title" aria-describedby="qr-account-login-description qr-account-login-status" onMouseDown={(event) => event.stopPropagation()}>
      <button className="iconButton qrAccountLoginClose" type="button" aria-label={text.close} onClick={onDismiss} disabled={!canCancel}><X aria-hidden="true" size={20} /></button>
      <div className="qrAccountLoginQrPane" aria-label={text.qrArea}><div className={`qrAccountLoginQrStage qrAccountLoginQrStage--${state}`}>{qrUrl ? <img src={qrUrl} alt={text.qrAlt} width={256} height={256} /> : state === "ready" ? <CheckCircle2 aria-hidden="true" size={52} /> : <div className="qrAccountLoginQrPlaceholder" aria-hidden="true">{state === "failed" || state === "expired" ? <AlertCircle size={40} /> : <LoaderCircle className="qrAccountLoginSpinner" size={40} />}</div>}</div><p>{text.qrHint}</p></div>
      <div className="qrAccountLoginContent"><div className="qrAccountLoginCopy"><h3 id="qr-account-login-title">{text.title}</h3><ol id="qr-account-login-description"><li>{text.stepOne}</li><li>{text.stepTwo}</li><li>{text.stepThree}</li></ol><div id="qr-account-login-status" className={`qrAccountLoginStatus qrAccountLoginStatus--${state}`} aria-live="polite"><StatusIcon state={state} /><span>{statusText(state, current.message)}</span></div>{failure ? <p className="qrAccountLoginError" role="alert">{failure}</p> : null}</div>
        <div className="qrAccountLoginActions">{current.phase === "recovering" ? <button className="ghostButton" type="button" onClick={onEndAndRetry} disabled={current.actionPending}>结束并重试</button> : <button className="ghostButton" type="button" onClick={onRefresh} disabled={!canRefresh}><RefreshCw aria-hidden="true" size={18} />{text.refresh}</button>}<button className="ghostButton" type="button" onClick={onDismiss} disabled={!canCancel}>{text.cancel}</button><button className="primaryButton" type="button" onClick={onUseExistingLogin} disabled={!canCancel}>{text.other}</button></div></div>
    </section>
  </div>;
}

export function QrAccountLoginDialog({ open, onCancel, onCompleted, onUseExistingLogin }: Props) {
  const rootRef = useRef<HTMLElement>(null);
  const completedRef = useRef(false);
  const controllerRef = useRef<ReturnType<typeof createQrAccountLoginController> | null>(null);
  if (!controllerRef.current) controllerRef.current = createQrAccountLoginController({ createRequestId: createQrLoginRequestId, start: addAccountWithQr, snapshot: getQrAccountLoginSnapshot, cancel: cancelQrAccountLogin, refresh: refreshQrAccountLogin, active: getActiveQrAccountLogin, operationStatus: getAccountOperationStatus });
  const controller = controllerRef.current;
  const [current, setCurrent] = useState<QrAccountLoginControllerState>(controller.getState());
  const state = asViewState(current.phase);
  const imageUrl = useQrImageUrl(current.imageBase64, imageStates.has(state));
  const stateRef = useRef(state);
  stateRef.current = state;

  useEffect(() => {
    if (!open) return;
    completedRef.current = false;
    const unsubscribe = controller.subscribe(setCurrent);
    void controller.start().catch(() => undefined);
    const timer = window.setInterval(() => void controller.poll(), 300);
    return () => { unsubscribe(); window.clearInterval(timer); void controller.cancel().catch(() => undefined); };
  }, [controller, open]);

  useEffect(() => {
    const completion = current.snapshot?.completion;
    if (!open || !completion?.result || completedRef.current) return;
    completedRef.current = true;
    const timer = window.setTimeout(() => void completeQrAccountLogin(completion, onCompleted, onCancel), 700);
    return () => window.clearTimeout(timer);
  }, [current.snapshot, onCancel, onCompleted, open]);

  useEffect(() => {
    if (!open) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    rootRef.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    return () => restoreQrDialogFocus(previous);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const keydown = (event: KeyboardEvent) => handleQrDialogKeyDown(
      event,
      cancellableStates.has(stateRef.current) && !current.actionPending,
      () => void requestCancel(),
      Array.from(rootRef.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []),
      document.activeElement instanceof HTMLElement ? document.activeElement : null,
    );
    document.addEventListener("keydown", keydown);
    return () => document.removeEventListener("keydown", keydown);
  }, [current.actionPending, open, state]);

  async function requestCancel() { if (current.actionPending || !cancellableStates.has(state)) return; try { await controller.cancel(); onCancel(); } catch { /* Keep recovery actions visible. */ } }
  async function requestRefresh() { if (current.actionPending || !["waitingForScan", "expired", "failed"].includes(state)) return; try { await controller.refresh(); } catch { /* Polling and fallback remain available. */ } }
  async function requestExistingLogin() { if (current.actionPending || !cancellableStates.has(state)) return; try { await controller.useExistingLogin(async () => { onCancel(); await onUseExistingLogin(); }); } catch { /* Never fall through before released=true. */ } }
  async function requestEndAndRetry() { if (current.actionPending) return; try { await controller.endAndRetry(); } catch { /* Recovery message remains visible. */ } }

  if (!open) return null;
  return <QrAccountLoginDialogView
    current={current}
    imageUrl={imageUrl}
    rootRef={rootRef}
    onDismiss={() => void requestCancel()}
    onRefresh={() => void requestRefresh()}
    onUseExistingLogin={() => void requestExistingLogin()}
    onEndAndRetry={() => void requestEndAndRetry()}
  />;
}
