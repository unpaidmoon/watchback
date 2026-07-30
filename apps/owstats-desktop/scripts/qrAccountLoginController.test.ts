import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createQrAccountLoginController, createQrLoginRequestId, qrLoginErrorMessage } from "../src/features/command-center/useQrAccountLogin";
import type { QrAccountLoginSnapshot } from "../src/shared/types";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => { resolve = nextResolve; });
  return { promise, resolve };
}

const terminalSnapshot = (sessionId: string, phase: "failed" | "expired", requestId: string): QrAccountLoginSnapshot => ({
  requestId,
  sessionId,
  generation: 1,
  phase,
  message: "请重试",
  revision: 1,
  imageBase64: null,
  settled: true,
  released: true,
  completion: null,
});

const activeSnapshot = (
  sessionId: string,
  revision: number,
  imageBase64 = `image-${revision}`,
): QrAccountLoginSnapshot => ({
  requestId: `request-${sessionId}`,
  sessionId,
  generation: 1,
  phase: "waitingForScan",
  message: "等待扫码",
  revision,
  imageBase64,
  settled: false,
  released: false,
  completion: null,
});

{
  assert.equal(createQrLoginRequestId(() => "stable-request"), "stable-request");
  assert.equal(qrLoginErrorMessage("qr_browser_not_found", "fallback"), "未找到 Chrome 或 Edge，请安装受支持的浏览器或使用其他登录方式。");
  assert.equal(qrLoginErrorMessage("unknown_code", "固定安全回退文案"), "固定安全回退文案");
  const dialog = readFileSync(new URL("../src/features/command-center/QrAccountLoginDialog.tsx", import.meta.url), "utf8");
  assert.match(dialog, /createQrAccountLoginController\(/, "生产弹窗必须由已测试控制器驱动");
  assert.match(dialog, /controller\.useExistingLogin/, "生产回退必须经过控制器的 settle-and-wait 路径");
}

{
  const observed: string[] = [];
  const controller = createQrAccountLoginController({
    createRequestId: () => "listener-first",
    start: async () => ({ requestId: "listener-first", sessionId: "listener-session", reused: false }),
    snapshot: async () => ({ ...activeSnapshot("listener-session", 1), requestId: "listener-first" }),
    cancel: async (sessionId) => ({ sessionId, terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  const unsubscribe = controller.subscribe((next) => observed.push(`${next.phase}:${next.sessionId ?? "none"}`));
  await controller.start();
  unsubscribe();
  assert.ok(observed.includes("waitingForScan:listener-session"), "启动前注册的监听器必须观察到首个权威快照");
}

{
  const settle = deferred<{ sessionId: string; terminalPhase: string; released: boolean }>();
  let addCount = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => "request-1",
    start: async () => ({ requestId: "request-1", sessionId: "session-1", reused: false }),
    snapshot: async () => terminalSnapshot("session-1", "failed", "request-1"),
    cancel: () => settle.promise,
    refresh: async () => ({ requestId: "request-2", sessionId: "session-2", reused: false }),
  });
  await controller.start();
  const first = controller.useExistingLogin(async () => { addCount += 1; });
  const second = controller.useExistingLogin(async () => { addCount += 1; });
  await Promise.resolve();
  assert.equal(addCount, 0, "cancel/settle 未完成时绝不能调用 addAccount");
  settle.resolve({ sessionId: "session-1", terminalPhase: "cancelled", released: true });
  await Promise.all([first, second]);
  assert.equal(addCount, 1, "快速双击其他登录方式只能发起一次 addAccount");
}

{
  let refreshCount = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => `request-${refreshCount + 1}`,
    start: async () => ({ requestId: "request-1", sessionId: "session-1", reused: false }),
    snapshot: async () => terminalSnapshot("session-1", "expired", "request-1"),
    cancel: async () => ({ sessionId: "session-1", terminalPhase: "expired", released: true }),
    refresh: async (_sessionId, requestId) => {
      refreshCount += 1;
      return { requestId, sessionId: "session-2", reused: false };
    },
  });
  await controller.start();
  await controller.refresh();
  assert.equal(controller.getState().sessionId, "session-2", "failed/expired 刷新必须切换到后继会话");
  assert.equal(refreshCount, 1);
}

{
  let attempts = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => "request-1",
    start: async () => ({ requestId: "request-1", sessionId: "session-1", reused: false }),
    snapshot: async () => {
      attempts += 1;
      if (attempts <= 2) throw new Error("IPC unavailable");
      return terminalSnapshot("session-1", "failed", "request-1");
    },
    cancel: async () => ({ sessionId: "session-1", terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "request-2", sessionId: "session-2", reused: false }),
  }, { pollFailureThreshold: 2 });
  await controller.start();
  await controller.poll();
  assert.equal(controller.getState().phase, "recovering");
  await controller.poll();
  assert.equal(controller.getState().phase, "failed", "轮询恢复后必须重新接受权威快照");
}

{
  const started = deferred<{ requestId: string; sessionId: string; reused: boolean }>();
  let cancelledSession: string | null = null;
  const controller = createQrAccountLoginController({
    createRequestId: () => "request-delayed",
    start: () => started.promise,
    snapshot: async (sessionId) => terminalSnapshot(sessionId, "failed", "request-delayed"),
    cancel: async (sessionId) => {
      cancelledSession = sessionId;
      return { sessionId, terminalPhase: "cancelled", released: true };
    },
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  const starting = controller.start();
  const closing = controller.cancel();
  started.resolve({ requestId: "request-delayed", sessionId: "session-delayed", reused: false });
  await Promise.all([starting, closing]);
  assert.equal(cancelledSession, "session-delayed", "关闭期间启动完成的后端会话也必须被终结");
  assert.equal(controller.getState().sessionId, null, "settled 后不能继续接受旧会话轮询结果");
}

{
  let starts = 0;
  let adds = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => "unknown-start",
    start: async () => { starts += 1; throw new Error("response lost"); },
    active: async () => null,
    operationStatus: async () => "account",
    snapshot: async () => terminalSnapshot("unused", "failed", "unknown-start"),
    cancel: async () => ({ sessionId: "unused", terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  await assert.rejects(controller.start());
  await assert.rejects(controller.useExistingLogin(async () => { adds += 1; }));
  assert.equal(starts, 2, "未知 start 结果必须用同一 requestId 重试一次");
  assert.equal(adds, 0, "active 未知且 operation 非 idle 时不得回退 addAccount");
}

{
  const controller = createQrAccountLoginController({
    createRequestId: () => "cancel-failure",
    start: async () => ({ requestId: "cancel-failure", sessionId: "session-cancel", reused: false }),
    snapshot: async () => terminalSnapshot("session-cancel", "failed", "cancel-failure"),
    cancel: async () => { throw new Error("cancel failed"); },
    refresh: async () => ({ requestId: "next", sessionId: "next", reused: false }),
  });
  await controller.start();
  await assert.rejects(controller.cancel());
  assert.notEqual(controller.getState().phase, "cancelling");
  assert.match(controller.getState().recoveryMessage ?? "", /重试/);
  await controller.refresh();
  assert.equal(controller.getState().sessionId, "next", "取消失败后仍须保留刷新或恢复操作能力");
}

{
  const response = deferred<QrAccountLoginSnapshot>();
  let polls = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => "poll-single-flight",
    start: async () => ({ requestId: "poll-single-flight", sessionId: "session-poll", reused: false }),
    snapshot: () => { polls += 1; return response.promise; },
    cancel: async () => ({ sessionId: "session-poll", terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "next", sessionId: "next", reused: false }),
  });
  const starting = controller.start();
  while (polls === 0) await Promise.resolve();
  const concurrent = controller.poll();
  assert.equal(polls, 1, "同一 epoch 的轮询必须单飞");
  response.resolve({ ...terminalSnapshot("session-poll", "failed", "poll-single-flight"), settled: true, released: false });
  await Promise.all([starting, concurrent]);
  assert.equal(controller.getState().phase, "starting", "未 settled/released 的伪终态不得发布");
}

{
  const controller = createQrAccountLoginController({
    createRequestId: () => "refresh-failure",
    start: async () => ({ requestId: "refresh-failure", sessionId: "session-refresh", reused: false }),
    snapshot: async () => terminalSnapshot("session-refresh", "expired", "refresh-failure"),
    cancel: async () => ({ sessionId: "session-refresh", terminalPhase: "cancelled", released: true }),
    refresh: async () => { throw new Error("refresh failed"); },
  });
  await controller.start();
  await assert.rejects(controller.refresh());
  assert.equal(controller.getState().phase, "expired", "刷新失败必须恢复旧终态以允许重试");
  assert.match(controller.getState().recoveryMessage ?? "", /刷新二维码失败/);
}

{
  const seenRequestIds: string[] = [];
  const controller = createQrAccountLoginController({
    createRequestId: () => "response-lost",
    start: async (requestId) => { seenRequestIds.push(requestId); throw new Error("response lost"); },
    active: async (requestId) => ({ requestId, sessionId: "adopted-session", generation: 2 }),
    snapshot: async () => ({ ...activeSnapshot("adopted-session", 3), requestId: "response-lost", generation: 2 }),
    cancel: async (sessionId) => ({ sessionId, terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  await controller.start();
  assert.deepEqual(seenRequestIds, ["response-lost", "response-lost"], "响应丢失后只能用同一 requestId 重试");
  assert.equal(controller.getState().sessionId, "adopted-session", "active 查询找到的会话必须被接管");
  assert.equal(controller.getState().phase, "waitingForScan");
}

{
  const stalePoll = deferred<QrAccountLoginSnapshot>();
  let snapshots = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => "out-of-order",
    start: async () => ({ requestId: "out-of-order", sessionId: "old-session", reused: false }),
    snapshot: async () => {
      snapshots += 1;
      if (snapshots === 1) return { ...activeSnapshot("old-session", 5), requestId: "out-of-order" };
      return stalePoll.promise;
    },
    cancel: async (sessionId) => ({ sessionId, terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "out-of-order", sessionId: "new-session", reused: false }),
  });
  await controller.start();
  const oldPoll = controller.poll();
  await controller.refresh();
  stalePoll.resolve({ ...terminalSnapshot("old-session", "failed", "out-of-order"), revision: 99 });
  await oldPoll;
  assert.equal(controller.getState().sessionId, "new-session");
  assert.equal(controller.getState().phase, "loadingQr", "旧 epoch 的晚到终态不得覆盖刷新后的会话");
}

{
  const refreshResult = deferred<{ requestId: string; sessionId: string; reused: boolean }>();
  const controller = createQrAccountLoginController({
    createRequestId: () => "refresh-image",
    start: async () => ({ requestId: "refresh-image", sessionId: "old-image-session", reused: false }),
    snapshot: async () => ({ ...activeSnapshot("old-image-session", 1, "old-image"), requestId: "refresh-image" }),
    cancel: async (sessionId) => ({ sessionId, terminalPhase: "cancelled", released: true }),
    refresh: () => refreshResult.promise,
  });
  await controller.start();
  const refreshing = controller.refresh();
  await Promise.resolve();
  assert.equal(controller.getState().imageBase64, "old-image", "后继会话确认前应保持旧二维码，避免空白跳变");
  refreshResult.resolve({ requestId: "refresh-image", sessionId: "new-image-session", reused: false });
  await refreshing;
  assert.equal(controller.getState().sessionId, "new-image-session");
  assert.equal(controller.getState().imageBase64, null, "后继会话确认后必须释放旧二维码");
}

{
  let request = 0;
  const starts: string[] = [];
  const controller = createQrAccountLoginController({
    createRequestId: () => `reopen-${++request}`,
    start: async (requestId) => {
      starts.push(requestId);
      return { requestId, sessionId: `session-${requestId}`, reused: false };
    },
    snapshot: async (sessionId) => terminalSnapshot(sessionId, "failed", sessionId.replace(/^session-/, "")),
    cancel: async (sessionId) => ({ sessionId, terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  await controller.start();
  await controller.cancel();
  await controller.start();
  assert.deepEqual(starts, ["reopen-1", "reopen-2"]);
  assert.equal(controller.getState().sessionId, "session-reopen-2", "关闭完成后重开必须绑定新会话");
}

{
  const cancellation = deferred<{ sessionId: string; terminalPhase: string; released: boolean }>();
  const starts: string[] = [];
  let request = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => `rapid-${++request}`,
    start: async (requestId) => {
      starts.push(requestId);
      return { requestId, sessionId: `session-${requestId}`, reused: false };
    },
    snapshot: async (sessionId) => terminalSnapshot(sessionId, "failed", sessionId.replace(/^session-/, "")),
    cancel: () => cancellation.promise,
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  await controller.start();
  const closing = controller.cancel();
  const reopening = controller.start();
  await Promise.resolve();
  assert.deepEqual(starts, ["rapid-1"], "取消尚未 released 时不得创建第二会话");
  cancellation.resolve({ sessionId: "session-rapid-1", terminalPhase: "cancelled", released: true });
  await Promise.all([closing, reopening]);
  assert.deepEqual(starts, ["rapid-1", "rapid-2"]);
  assert.equal(controller.getState().sessionId, "session-rapid-2");
}

{
  const snapshots = [
    activeSnapshot("revision-session", 5, "newer-image"),
    activeSnapshot("revision-session", 4, "older-image"),
  ];
  const controller = createQrAccountLoginController({
    createRequestId: () => "request-revision-session",
    start: async () => ({ requestId: "request-revision-session", sessionId: "revision-session", reused: false }),
    snapshot: async () => snapshots.shift()!,
    cancel: async (sessionId) => ({ sessionId, terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  await controller.start();
  await controller.poll();
  assert.equal(controller.getState().revision, 5);
  assert.equal(controller.getState().imageBase64, "newer-image", "同一 generation 的旧 revision 不得回滚二维码");
}

{
  let snapshotAttempt = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => "expected-request",
    start: async () => ({ requestId: "expected-request", sessionId: "shared-session", reused: false }),
    snapshot: async () => {
      snapshotAttempt += 1;
      return {
        ...activeSnapshot("shared-session", snapshotAttempt === 1 ? 99 : 1),
        requestId: snapshotAttempt === 1 ? "foreign-request" : "expected-request",
      };
    },
    cancel: async (sessionId) => ({ sessionId, terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  await controller.start();
  assert.equal(controller.getState().revision, -1, "同 session 但 requestId 不匹配的快照也必须拒绝");
  await controller.poll();
  assert.equal(controller.getState().revision, 1);
}

{
  const controller = createQrAccountLoginController({
    createRequestId: () => "released-terminal",
    start: async () => ({ requestId: "released-terminal", sessionId: "released-session", reused: false }),
    snapshot: async () => ({
      ...terminalSnapshot("released-session", "failed", "released-terminal"),
      imageBase64: "must-be-released",
    }),
    cancel: async (sessionId) => ({ sessionId, terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  await controller.start();
  assert.equal(controller.getState().phase, "failed", "只有 settled+released 的终态才可发布");
  assert.equal(controller.getState().imageBase64, null, "已发布终态不得保留二维码数据");
}

{
  let cancels = 0;
  let starts = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => `recover-${starts + 1}`,
    start: async (requestId) => {
      starts += 1;
      return { requestId, sessionId: `recover-session-${starts}`, reused: false };
    },
    snapshot: async () => { throw new Error("poll unavailable"); },
    cancel: async (sessionId) => {
      cancels += 1;
      return { sessionId, terminalPhase: "cancelled", released: true };
    },
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  }, { pollFailureThreshold: 1 });
  await controller.start();
  assert.equal(controller.getState().phase, "recovering");
  await controller.endAndRetry();
  assert.equal(cancels, 1, "recovering 必须能结束原会话");
  assert.equal(starts, 2, "结束并重试必须在 released 后创建后继会话");
}

{
  const cancellation = deferred<{ sessionId: string; terminalPhase: string; released: boolean }>();
  let cancelCalls = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => "double-cancel",
    start: async () => ({ requestId: "double-cancel", sessionId: "double-cancel-session", reused: false }),
    snapshot: async () => terminalSnapshot("double-cancel-session", "failed", "double-cancel"),
    cancel: () => { cancelCalls += 1; return cancellation.promise; },
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  await controller.start();
  const first = controller.cancel();
  const second = controller.cancel();
  assert.equal(cancelCalls, 1, "快速双击取消必须单飞");
  cancellation.resolve({ sessionId: "double-cancel-session", terminalPhase: "cancelled", released: true });
  await Promise.all([first, second]);
  assert.equal(controller.getState().recoveryMessage, null, "预期取消不得显示添加账号失败");
}

{
  const refreshed = deferred<{ requestId: string; sessionId: string; reused: boolean }>();
  let refreshCalls = 0;
  const controller = createQrAccountLoginController({
    createRequestId: () => "double-refresh",
    start: async () => ({ requestId: "double-refresh", sessionId: "double-refresh-session", reused: false }),
    snapshot: async () => terminalSnapshot("double-refresh-session", "expired", "double-refresh"),
    cancel: async (sessionId) => ({ sessionId, terminalPhase: "cancelled", released: true }),
    refresh: () => { refreshCalls += 1; return refreshed.promise; },
  });
  await controller.start();
  const first = controller.refresh();
  const second = controller.refresh();
  assert.equal(refreshCalls, 1, "快速双击刷新必须单飞");
  refreshed.resolve({ requestId: "double-refresh", sessionId: "double-refresh-next", reused: false });
  await Promise.all([first, second]);
}

{
  const snapshots = [
    { ...activeSnapshot("generation-session", 2, "generation-two"), requestId: "generation-request", generation: 2 },
    { ...activeSnapshot("generation-session", 99, "generation-one"), requestId: "generation-request", generation: 1 },
  ];
  const controller = createQrAccountLoginController({
    createRequestId: () => "generation-request",
    start: async () => ({ requestId: "generation-request", sessionId: "generation-session", reused: false }),
    snapshot: async () => snapshots.shift()!,
    cancel: async (sessionId) => ({ sessionId, terminalPhase: "cancelled", released: true }),
    refresh: async () => ({ requestId: "unused", sessionId: "unused", reused: false }),
  });
  await controller.start();
  await controller.poll();
  assert.equal(controller.getState().generation, 2);
  assert.equal(controller.getState().imageBase64, "generation-two", "旧 generation 即使 revision 更高也不得覆盖当前状态");
}
