import type { AccountOperationStatus, QrAccountLoginActiveResult, QrAccountLoginSettleResult, QrAccountLoginSnapshot, QrAccountLoginStartResult, QrLoginControllerPhase } from "../../shared/types";
import { runExistingLoginFallback } from "./accountOperationIsolation";

export type QrAccountLoginControllerDependencies = {
  createRequestId: () => string;
  start: (requestId: string) => Promise<QrAccountLoginStartResult>;
  snapshot: (sessionId: string) => Promise<QrAccountLoginSnapshot>;
  cancel: (sessionId: string) => Promise<QrAccountLoginSettleResult>;
  refresh: (sessionId: string, requestId: string) => Promise<QrAccountLoginStartResult>;
  active?: (requestId: string) => Promise<QrAccountLoginActiveResult | null>;
  operationStatus?: () => Promise<AccountOperationStatus>;
};
export type QrAccountLoginControllerState = { requestId: string | null; sessionId: string | null; generation: number; revision: number; phase: QrLoginControllerPhase; message: string | null; imageBase64: string | null; snapshot: QrAccountLoginSnapshot | null; pollFailures: number; actionPending: boolean; recoveryMessage: string | null };
const initialState: QrAccountLoginControllerState = { requestId: null, sessionId: null, generation: 0, revision: -1, phase: "starting", message: null, imageBase64: null, snapshot: null, pollFailures: 0, actionPending: false, recoveryMessage: null };
const CANCEL_RECOVERY = "未能确认二维码登录已结束，请重试取消或使用“结束并重试”。";
const START_RECOVERY = "无法确认二维码登录是否已启动，请先结束当前登录再重试。";

const QR_ERROR_MESSAGES: Readonly<Record<string, string>> = {
  qr_browser_not_found: "未找到 Chrome 或 Edge，请安装受支持的浏览器或使用其他登录方式。",
  qr_browser_start_failed: "无法启动安全登录环境，请重试或使用其他登录方式。",
  qr_debug_endpoint_timeout: "登录页面启动超时，请刷新或使用其他登录方式。",
  qr_capture_timeout: "二维码已过期，请刷新后重试。",
  qr_expired: "二维码已过期，请刷新后重试。",
  qr_worker_failed: "登录任务异常结束，请结束并重试或使用其他登录方式。",
};

export function qrLoginErrorMessage(errorCode: string | null | undefined, fallback: string) {
  return (errorCode && QR_ERROR_MESSAGES[errorCode]) || fallback;
}

export function createQrLoginRequestId(randomUuid?: () => string) { const uuid = randomUuid ?? globalThis.crypto?.randomUUID?.bind(globalThis.crypto); return uuid ? uuid() : `qr-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}-${Math.random().toString(36).slice(2)}`; }

export function createQrAccountLoginController(dependencies: QrAccountLoginControllerDependencies, options: { pollFailureThreshold?: number } = {}) {
  const threshold = options.pollFailureThreshold ?? 3;
  let state = { ...initialState };
  let action: Promise<unknown> | null = null;
  let actionKind: "start" | "refresh" | "cancel" | "fallback" | null = null;
  let pollFlight: Promise<void> | null = null;
  let epoch = 0;
  const listeners = new Set<(value: QrAccountLoginControllerState) => void>();
  const active = dependencies.active ?? (async () => null);
  const operationStatus = dependencies.operationStatus ?? (async () => "account" as const);
  const publish = (patch: Partial<QrAccountLoginControllerState>) => { state = { ...state, ...patch }; listeners.forEach((listener) => listener(state)); };
  const runAction = async <T>(kind: NonNullable<typeof actionKind>, work: () => Promise<T>): Promise<T> => {
    if (action) { if (actionKind === kind) return action as Promise<T>; try { await action; } catch { /* next action performs recovery */ } }
    const run = work(); action = run; actionKind = kind; publish({ actionPending: true });
    try { return await run; } finally { if (action === run) { action = null; actionKind = null; publish({ actionPending: false }); } }
  };

  const acceptSnapshot = (snapshot: QrAccountLoginSnapshot, expectedEpoch: number) => {
    if (expectedEpoch !== epoch || snapshot.requestId !== state.requestId || snapshot.sessionId !== state.sessionId) return;
    if (snapshot.generation < state.generation || (snapshot.generation === state.generation && snapshot.revision <= state.revision)) return;
    const terminal = ["ready", "failed", "expired", "cancelled"].includes(snapshot.phase);
    if (terminal && (!snapshot.settled || !snapshot.released)) return;
    const message = snapshot.phase === "failed"
      ? qrLoginErrorMessage(snapshot.completion?.errorCode, snapshot.message)
      : snapshot.message;
    publish({ requestId: snapshot.requestId, generation: snapshot.generation, revision: snapshot.revision, phase: snapshot.phase as QrLoginControllerPhase, message, imageBase64: terminal ? null : snapshot.imageBase64, snapshot, pollFailures: 0, recoveryMessage: null });
  };
  const poll = () => {
    if (pollFlight || actionKind === "refresh" || !state.sessionId) return pollFlight ?? Promise.resolve();
    const sessionId = state.sessionId; const expectedEpoch = epoch;
    const run = dependencies.snapshot(sessionId).then((snapshot) => acceptSnapshot(snapshot, expectedEpoch)).catch(() => {
      if (expectedEpoch !== epoch || sessionId !== state.sessionId) return;
      const failures = state.pollFailures + 1;
      publish({ pollFailures: failures, ...(failures >= threshold ? { phase: "recovering" as const, message: "与登录服务的连接暂时中断。", recoveryMessage: "请结束当前登录并重试。" } : {}) });
    }).finally(() => { if (pollFlight === run) pollFlight = null; });
    pollFlight = run; return run;
  };
  const recoverStart = async (requestId: string) => {
    try { return await dependencies.start(requestId); } catch {
      try { return await dependencies.start(requestId); } catch {
        let activeSession: QrAccountLoginActiveResult | null = null;
        try { activeSession = await active(requestId); } catch { /* Treat an unavailable recovery query as unknown state. */ }
        if (activeSession) return { requestId: activeSession.requestId, sessionId: activeSession.sessionId, reused: true };
        let status: AccountOperationStatus | null = null;
        try { status = await operationStatus(); } catch { /* Unknown operation state must fail closed. */ }
        if (status === "idle") {
          const message = "二维码登录启动失败，请结束并重试。";
          publish({ phase: "recovering", recoveryMessage: message, message });
          throw new Error(message);
        }
        publish({ phase: "recovering", recoveryMessage: START_RECOVERY, message: START_RECOVERY });
        throw new Error(START_RECOVERY);
      }
    }
  };
  const start = () => runAction("start", async () => {
    const requestId = !state.requestId || ["cancelled", "ready", "failed", "expired"].includes(state.phase) ? dependencies.createRequestId() : state.requestId;
    epoch += 1; publish({ ...initialState, requestId, actionPending: true });
    const result = await recoverStart(requestId); publish({ requestId: result.requestId, sessionId: result.sessionId }); await poll();
  });
  const refresh = () => runAction("refresh", async () => {
    const sessionId = state.sessionId; if (!sessionId) throw new Error("没有可刷新的二维码登录会话。");
    const previous = { phase: state.phase, message: state.message, imageBase64: state.imageBase64, snapshot: state.snapshot };
    epoch += 1; const requestId = state.requestId ?? dependencies.createRequestId(); publish({ phase: "loadingQr", message: null, recoveryMessage: null });
    try { const result = await dependencies.refresh(sessionId, requestId); publish({ requestId: result.requestId, sessionId: result.sessionId, generation: 0, revision: -1, imageBase64: null, snapshot: null, pollFailures: 0 }); }
    catch (error) { publish({ ...previous, recoveryMessage: "刷新二维码失败，请重试或使用其他登录方式。" }); throw error; }
  });
  const cancel = () => runAction("cancel", async () => {
    let sessionId = state.sessionId;
    if (!sessionId && state.requestId) {
      try {
        const recovered = await active(state.requestId);
        if (recovered) { sessionId = recovered.sessionId; publish({ sessionId, generation: recovered.generation }); }
      } catch { /* Fall through to the global operation check. */ }
    }
    if (!sessionId) { let status: AccountOperationStatus | null = null; try { status = await operationStatus(); } catch { /* Fail closed below. */ } if (status !== "idle") { publish({ phase: "recovering", message: CANCEL_RECOVERY, recoveryMessage: CANCEL_RECOVERY }); throw new Error(CANCEL_RECOVERY); } return; }
    const previous = state.phase; publish({ phase: "cancelling", recoveryMessage: null });
    try { const settled = await dependencies.cancel(sessionId); if (!settled.released) throw new Error(CANCEL_RECOVERY); epoch += 1; publish({ phase: "cancelled", sessionId: null, imageBase64: null, snapshot: null }); }
    catch (error) { publish({ phase: previous, message: CANCEL_RECOVERY, recoveryMessage: CANCEL_RECOVERY }); throw error; }
  });
  const useExistingLogin = (addAccount: () => Promise<void>) => runAction("fallback", async () => {
    const previous = state.phase;
    let sessionId = state.sessionId;
    if (!sessionId && state.requestId) {
      try {
        const recovered = await active(state.requestId);
        if (recovered) { sessionId = recovered.sessionId; publish({ sessionId, generation: recovered.generation }); }
      } catch { /* The operation-status check below remains fail-closed. */ }
    }
    try { await runExistingLoginFallback({ settle: async () => { if (sessionId) { publish({ phase: "cancelling" }); return dependencies.cancel(sessionId); } const status = await operationStatus(); return { released: status === "idle" }; }, addAccount }); epoch += 1; publish({ phase: "cancelled", sessionId: null, imageBase64: null, snapshot: null }); }
    catch (error) { publish({ phase: previous === "cancelling" ? "failed" : previous, message: CANCEL_RECOVERY, recoveryMessage: CANCEL_RECOVERY }); throw error; }
  });
  const endAndRetry = async () => { await cancel(); await start(); };
  return { getState: () => state, subscribe(listener: (value: QrAccountLoginControllerState) => void) { listeners.add(listener); return () => listeners.delete(listener); }, start, poll, refresh, cancel, useExistingLogin, endAndRetry };
}
