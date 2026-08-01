import assert from "node:assert/strict";
import { createElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { TargetRecoveryPanel } from "../src/pages/OnboardingPage";
import type { LoginTargetStatus } from "../src/shared/types";

type ElementWithProps = ReactElement<Record<string, unknown>>;

const readyTarget: LoginTargetStatus = {
  state: "ready",
  detectedBnet: "Current#1234",
  currentBnet: "Current#1234",
  message: "目标已确认",
  syncAllowed: true,
  canConfirmSwitch: false,
  canRetry: false,
};

function visitElements(node: ReactNode, visitor: (element: ElementWithProps) => void) {
  if (Array.isArray(node)) {
    for (const child of node) visitElements(child, visitor);
    return;
  }
  if (!node || typeof node !== "object" || !("props" in node)) return;
  const element = node as ElementWithProps;
  visitor(element);
  visitElements(element.props.children as ReactNode, visitor);
}

function renderPanel(targetStatus: LoginTargetStatus | null, options?: {
  targetAction?: "confirm" | "keep" | "retry" | "clear" | null;
  disabled?: boolean;
  calls?: string[];
}) {
  const calls = options?.calls ?? [];
  const primaryActionRef = { current: null };
  const element = TargetRecoveryPanel({
    targetStatus,
    fallbackMessage: "正在识别目标账号",
    targetAction: options?.targetAction ?? null,
    disabled: options?.disabled ?? false,
    primaryActionRef,
    onConfirmSwitch: () => calls.push("confirm"),
    onKeepCurrent: () => calls.push("keep"),
    onRetryBinding: () => calls.push("retry"),
    onClear: () => calls.push("clear"),
  });
  const buttons: ElementWithProps[] = [];
  visitElements(element, (candidate) => {
    if (candidate.type === "button") buttons.push(candidate);
  });
  return {
    calls,
    element,
    html: renderToStaticMarkup(createElement(() => element)),
    buttons,
    primaryActionRef,
  };
}

function buttonByLabel(buttons: ElementWithProps[], label: string) {
  const button = buttons.find((candidate) => candidate.props.children === label);
  assert.ok(button, `expected action button: ${label}`);
  return button;
}

const loading = renderPanel(null);
assert.ok(loading.html.includes('aria-labelledby="sync-target-title"'));
assert.ok(loading.html.includes('aria-describedby="sync-target-message"'));
assert.ok(loading.html.includes('aria-live="polite"'));
assert.ok(loading.html.includes("正在识别目标账号"));
assert.ok(loading.html.includes("正在确认目标状态…"));
assert.equal(loading.buttons.length, 1);
assert.equal(buttonByLabel(loading.buttons, "清除").props.disabled, true);

const ready = renderPanel(readyTarget);
assert.ok(ready.html.includes("当前目标已确认，可同步"));
assert.doesNotMatch(ready.html, /当前目标与检测到的登录账号/);
assert.equal(ready.buttons.length, 1);
assert.equal(buttonByLabel(ready.buttons, "清除").props.disabled, false);
buttonByLabel(ready.buttons, "清除").props.onClick!();
assert.deepEqual(ready.calls, ["clear"]);

const switchRequired = renderPanel({
  ...readyTarget,
  state: "switchRequired",
  currentBnet: "Current#1234",
  detectedBnet: "Detected＃5678",
  message: "检测到不同账号",
  syncAllowed: false,
  canConfirmSwitch: true,
});
assert.ok(switchRequired.html.includes("当前目标与检测到的登录账号"));
assert.ok(switchRequired.html.includes("Current#1234"));
assert.ok(switchRequired.html.includes("Detected＃5678"));
assert.ok(switchRequired.html.includes("检测到其他登录账号，请选择目标"));
assert.equal(switchRequired.buttons.length, 3);
buttonByLabel(switchRequired.buttons, "切换到此账号").props.onClick!();
buttonByLabel(switchRequired.buttons, "保留原目标").props.onClick!();
buttonByLabel(switchRequired.buttons, "清除").props.onClick!();
assert.deepEqual(switchRequired.calls, ["confirm", "keep", "clear"]);
assert.equal(
  (buttonByLabel(switchRequired.buttons, "切换到此账号") as ElementWithProps & { ref: unknown }).ref,
  switchRequired.primaryActionRef,
  "切换操作必须绑定主操作焦点 ref",
);

const switchUnavailable = renderPanel({
  ...readyTarget,
  state: "switchRequired",
  syncAllowed: false,
  canConfirmSwitch: false,
});
assert.equal(buttonByLabel(switchUnavailable.buttons, "切换到此账号").props.disabled, true);
assert.equal(buttonByLabel(switchUnavailable.buttons, "保留原目标").props.disabled, false);

const retryableFailure = renderPanel({
  ...readyTarget,
  state: "bindFailed",
  message: "绑定请求失败",
  syncAllowed: false,
  canRetry: true,
});
assert.ok(retryableFailure.html.includes("目标绑定失败"));
assert.equal(retryableFailure.buttons.length, 2);
buttonByLabel(retryableFailure.buttons, "重新绑定").props.onClick!();
assert.deepEqual(retryableFailure.calls, ["retry"]);
assert.equal(
  (buttonByLabel(retryableFailure.buttons, "重新绑定") as ElementWithProps & { ref: unknown }).ref,
  retryableFailure.primaryActionRef,
  "重试操作必须绑定主操作焦点 ref",
);

const terminalFailure = renderPanel({
  ...readyTarget,
  state: "bindFailed",
  currentBnet: null,
  message: "无法绑定目标账号",
  syncAllowed: false,
  canRetry: false,
});
assert.equal(terminalFailure.buttons.length, 1);
assert.equal(buttonByLabel(terminalFailure.buttons, "清除").props.disabled, true);

const unidentified = renderPanel({
  ...readyTarget,
  state: "unidentified",
  currentBnet: null,
  detectedBnet: null,
  message: "尚未识别登录账号",
  syncAllowed: false,
});
assert.ok(unidentified.html.includes("同步前需要处理目标状态"));
assert.equal(buttonByLabel(unidentified.buttons, "清除").props.disabled, true);

const busySwitch = renderPanel({
  ...readyTarget,
  state: "switchRequired",
  detectedBnet: "Other#5678",
  syncAllowed: false,
  canConfirmSwitch: true,
}, { targetAction: "confirm", disabled: true });
assert.ok(busySwitch.html.includes('aria-busy="true"'));
assert.equal(buttonByLabel(busySwitch.buttons, "切换中…").props.disabled, true);
assert.equal(buttonByLabel(busySwitch.buttons, "保留原目标").props.disabled, true);
assert.equal(buttonByLabel(busySwitch.buttons, "清除").props.disabled, true);
