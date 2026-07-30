import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  QrAccountLoginDialogView,
  completeQrAccountLogin,
  createQrImageObjectUrl,
  handleQrDialogKeyDown,
  restoreQrDialogFocus,
  type QrAccountLoginDialogViewProps,
} from "../src/features/command-center/QrAccountLoginDialog";
import type { QrAccountLoginControllerState } from "../src/features/command-center/useQrAccountLogin";
import {
  ACCOUNT_OPERATION_STATUSES,
  type AccountOperationStatus,
} from "../src/shared/types";

const state = (
  phase: QrAccountLoginControllerState["phase"],
  patch: Partial<QrAccountLoginControllerState> = {},
): QrAccountLoginControllerState => ({
  requestId: "request-dialog",
  sessionId: "session-dialog",
  generation: 1,
  revision: 4,
  phase,
  message: null,
  imageBase64: "cWItcG5n",
  snapshot: null,
  pollFailures: 0,
  actionPending: false,
  recoveryMessage: null,
  ...patch,
});

const noop = () => undefined;
const props = (
  current: QrAccountLoginControllerState,
  patch: Partial<QrAccountLoginDialogViewProps> = {},
): QrAccountLoginDialogViewProps => ({
  current,
  imageUrl: current.imageBase64 ? "blob:qr-test" : null,
  rootRef: { current: null },
  onDismiss: noop,
  onRefresh: noop,
  onUseExistingLogin: noop,
  onEndAndRetry: noop,
  ...patch,
});

function walk(node: ReactNode, visit: (element: ReactElement<Record<string, unknown>>) => void) {
  if (Array.isArray(node)) {
    node.forEach((child) => walk(child, visit));
    return;
  }
  if (!node || typeof node !== "object" || !("props" in node)) return;
  const element = node as ReactElement<Record<string, unknown>>;
  visit(element);
  walk(element.props.children as ReactNode, visit);
}

function findElement(root: ReactElement, predicate: (props: Record<string, unknown>) => boolean) {
  let match: ReactElement<Record<string, unknown>> | null = null;
  walk(root, (element) => {
    if (!match && predicate(element.props)) match = element;
  });
  assert.ok(match, "expected dialog element was not rendered");
  return match;
}

{
  const waiting = renderToStaticMarkup(createElement(QrAccountLoginDialogView, props(state("waitingForScan"))));
  assert.match(waiting, /role="dialog"/);
  assert.match(waiting, /aria-live="polite"/);
  assert.match(waiting, /<img[^>]+blob:qr-test/);
  assert.doesNotMatch(waiting, /data:image\//, "二维码不得以内联 data URL 长时间驻留在 DOM");
  assert.doesNotMatch(waiting, /role="alert"/);

  const failed = renderToStaticMarkup(createElement(QrAccountLoginDialogView, props(state("failed", {
    recoveryMessage: "登录连接中断，请重试",
  }))));
  assert.match(failed, /role="alert"[^>]*>登录连接中断，请重试</);
  assert.doesNotMatch(failed, /<img/, "失败终态不能继续持有或显示旧二维码");
}

{
  const revoked: string[] = [];
  let createdType = "";
  let createdSize = 0;
  const objectUrl = createQrImageObjectUrl("cWItcG5n", {
    decode: () => new Uint8Array([1, 2, 3, 4]),
    createObjectUrl: (blob) => {
      createdType = blob.type;
      createdSize = blob.size;
      return "blob:qr-lifecycle";
    },
    revokeObjectUrl: (url) => revoked.push(url),
  });
  assert.equal(objectUrl.url, "blob:qr-lifecycle");
  assert.equal(createdType, "image/png");
  assert.equal(createdSize, 4);
  objectUrl.revoke();
  assert.deepEqual(revoked, ["blob:qr-lifecycle"], "替换、终态或卸载时必须撤销 Blob URL");
}

{
  for (const phase of ["saving", "ready"] as const) {
    const html = renderToStaticMarkup(createElement(QrAccountLoginDialogView, props(state(phase))));
    assert.doesNotMatch(html, /<img/, `${phase} 阶段必须释放二维码 DOM`);
    assert.match(html, /qrAccountLoginClose[^>]*disabled/);
    assert.match(html, /disabled=""[^>]*>取消<\/button>/);
    assert.match(html, /disabled=""[^>]*>其他登录方式<\/button>/);
  }
}

{
  let dismissals = 0;
  const dismiss = () => { dismissals += 1; };
  const tree = QrAccountLoginDialogView(props(state("waitingForScan"), { onDismiss: dismiss }));
  const backdrop = findElement(tree, (value) => value.className === "qrAccountLoginBackdrop");
  const close = findElement(tree, (value) => value["aria-label"] === "关闭二维码登录");
  const cancel = findElement(tree, (value) => value.children === "取消");
  (backdrop.props.onMouseDown as () => void)();
  (close.props.onClick as () => void)();
  (cancel.props.onClick as () => void)();
  let prevented = false;
  handleQrDialogKeyDown({ key: "Escape", preventDefault: () => { prevented = true; } }, true, dismiss);
  assert.equal(dismissals, 4, "遮罩、关闭、取消和 Esc 必须进入同一个 dismiss 路径");
  assert.equal(prevented, true);

  handleQrDialogKeyDown({ key: "Escape", preventDefault: () => assert.fail("saving must not consume Escape") }, false, dismiss);
  assert.equal(dismissals, 4, "不可取消阶段必须忽略 Esc");

  const focused: string[] = [];
  const first = { focus: () => focused.push("first") };
  const last = { focus: () => focused.push("last") };
  handleQrDialogKeyDown({ key: "Tab", shiftKey: true, preventDefault: noop }, true, dismiss, [first, last], first);
  handleQrDialogKeyDown({ key: "Tab", shiftKey: false, preventDefault: noop }, true, dismiss, [first, last], last);
  assert.deepEqual(focused, ["last", "first"], "Tab 和 Shift+Tab 必须约束在弹窗可用按钮内");

  let restored = 0;
  restoreQrDialogFocus({ isConnected: true, focus: () => { restored += 1; } });
  restoreQrDialogFocus({ isConnected: false, focus: () => { restored += 10; } });
  assert.equal(restored, 1, "关闭后只恢复仍连接的原焦点元素");
}

{
  const table = [
    { phase: "waitingForScan" as const, refresh: false, cancel: false, other: false },
    { phase: "confirming" as const, refresh: true, cancel: false, other: false },
    { phase: "capturing" as const, refresh: true, cancel: false, other: false },
    { phase: "expired" as const, refresh: false, cancel: false, other: false },
    { phase: "failed" as const, refresh: false, cancel: false, other: false },
    { phase: "saving" as const, refresh: true, cancel: true, other: true },
    { phase: "ready" as const, refresh: true, cancel: true, other: true },
  ];
  for (const expected of table) {
    const tree = QrAccountLoginDialogView(props(state(expected.phase)));
    const refresh = findElement(tree, (value) => value.children instanceof Array && value.children.some((child) => child === "刷新"));
    const cancel = findElement(tree, (value) => value.children === "取消");
    const other = findElement(tree, (value) => value.children === "其他登录方式");
    assert.equal(Boolean(refresh.props.disabled), expected.refresh, `${expected.phase} refresh rule`);
    assert.equal(Boolean(cancel.props.disabled), expected.cancel, `${expected.phase} cancel rule`);
    assert.equal(Boolean(other.props.disabled), expected.other, `${expected.phase} fallback rule`);
  }
}

{
  const expected = ["idle", "account", "foregroundSync", "backgroundQueued", "backgroundRunning", "query"] as const;
  assert.deepEqual(ACCOUNT_OPERATION_STATUSES, expected, "TS fixture must match get_operation_status camelCase strings exactly");
  const exhaustiveFixture: readonly AccountOperationStatus[] = expected;
  assert.equal(exhaustiveFixture.length, 6);
  const api = readFileSync(new URL("../src/shared/api.ts", import.meta.url), "utf8");
  assert.match(api, /invoke<AccountOperationStatus>\("get_operation_status"\)/);
}

{
  const sequence: string[] = [];
  const result = {
    kind: "added" as const,
    accountId: "account-a",
    battleTag: "Tester#1234",
    message: "账号已保存，资料将在稍后刷新",
    shouldOfferSwitch: false,
  };
  await completeQrAccountLogin(
    { sessionId: "ready-session", result, message: null, cancelled: false },
    async (message) => { sequence.push(`refresh:${message}`); },
    () => sequence.push("close"),
  );
  assert.deepEqual(sequence, ["refresh:账号已保存，资料将在稍后刷新", "close"], "成功和 committed warning 都必须先刷新账号列表再关闭");

  await assert.rejects(completeQrAccountLogin(
    { sessionId: "ready-session", result, message: null, cancelled: false },
    async () => { throw new Error("refresh failed"); },
    () => sequence.push("close-after-refresh-error"),
  ));
  assert.equal(sequence.at(-1), "close-after-refresh-error", "账号列表刷新异常也不能把用户困在完成弹窗");
}

{
  const css = readFileSync(new URL("../src/styles/features/command-center.css", import.meta.url), "utf8");
  assert.match(css, /\.qrAccountLoginQrStage\s*\{[\s\S]*?width:\s*256px;[\s\S]*?height:\s*256px;/, "二维码占位和图片必须共享固定舞台，状态切换不能跳动");
  assert.match(css, /@media \(max-width: 680px\)[\s\S]*?\.qrAccountLoginDialog\s*\{\s*grid-template-columns:\s*1fr;/, "窄屏弹窗必须切换单列布局");
  assert.match(css, /\.qrAccountLoginDialog\s*\{[\s\S]*?width:\s*min\(760px, 100%\);[\s\S]*?overflow:\s*auto;/, "弹窗不得制造横向溢出且须允许视口内滚动");
}
