import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NavLink } from "react-router-dom";
import { StaticRouter } from "react-router-dom/server";
import * as routeFocusModule from "../src/shared/page-shell/RouteFocusManager.tsx";

type RouteFocusLocation = {
  pathname: string;
  hash: string;
  key: string;
};

type RouteFocusRuntime = {
  getElementById: (id: string) => HTMLElement | null;
  requestAnimationFrame: (callback: (timestamp: number) => void) => number;
  cancelAnimationFrame: (handle: number) => void;
};

type ScheduleRouteFocus = (
  location: RouteFocusLocation,
  runtime: RouteFocusRuntime,
) => void | (() => void);

function assertEqual<T>(actual: T, expected: T, message: string) {
  if (actual !== expected) {
    throw new Error(`${message}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

function createTarget(options: { hasTabIndex?: boolean; nativeFocusable?: boolean } = {}) {
  const state = {
    tabIndex: options.hasTabIndex ? -1 : undefined as number | undefined,
    tabIndexWrites: [] as number[],
    scrollOptions: [] as ScrollIntoViewOptions[],
    focusOptions: [] as FocusOptions[],
  };

  const target = {
    hasAttribute(name: string) {
      return name === "tabindex" && options.hasTabIndex === true;
    },
    matches() {
      return options.nativeFocusable === true;
    },
    get tabIndex() {
      return state.tabIndex ?? -1;
    },
    set tabIndex(value: number) {
      state.tabIndex = value;
      state.tabIndexWrites.push(value);
    },
    scrollIntoView(scrollOptions: ScrollIntoViewOptions) {
      state.scrollOptions.push(scrollOptions);
    },
    focus(focusOptions: FocusOptions) {
      state.focusOptions.push(focusOptions);
    },
  } as unknown as HTMLElement;

  return { state, target };
}

function createRuntime(target: HTMLElement | null) {
  let nextHandle = 1;
  const callbacks = new Map<number, (timestamp: number) => void>();
  const state = {
    requested: [] as number[],
    cancelled: [] as number[],
    targetIds: [] as string[],
  };

  const runtime: RouteFocusRuntime = {
    getElementById(id) {
      state.targetIds.push(id);
      return target;
    },
    requestAnimationFrame(callback) {
      const handle = nextHandle++;
      state.requested.push(handle);
      callbacks.set(handle, callback);
      return handle;
    },
    cancelAnimationFrame(handle) {
      state.cancelled.push(handle);
      callbacks.delete(handle);
    },
  };

  return {
    flush(handle: number) {
      const callback = callbacks.get(handle);
      callbacks.delete(handle);
      callback?.(0);
    },
    runtime,
    state,
  };
}

const scheduleRouteFocus = (routeFocusModule as { scheduleRouteFocus?: ScheduleRouteFocus }).scheduleRouteFocus;
assertEqual(typeof scheduleRouteFocus, "function", "route focus behavior is exported for direct testing");

if (!scheduleRouteFocus) {
  throw new Error("route focus behavior is unavailable");
}

const ordinaryTarget = createTarget();
const ordinaryRuntime = createRuntime(ordinaryTarget.target);
scheduleRouteFocus({ pathname: "/onboarding", hash: "#local-data", key: "entry-a" }, ordinaryRuntime.runtime);
ordinaryRuntime.flush(ordinaryRuntime.state.requested[0]);
assertEqual(ordinaryRuntime.state.targetIds.join(","), "local-data", "hash resolves the matching target id");
assertEqual(ordinaryTarget.state.tabIndexWrites.join(","), "-1", "ordinary target becomes programmatically focusable");
assertEqual(ordinaryTarget.state.scrollOptions[0]?.block, "start", "hash target scrolls to the start edge");
assertEqual(ordinaryTarget.state.focusOptions[0]?.preventScroll, true, "hash target receives focus without a second scroll");

const explicitTarget = createTarget({ hasTabIndex: true });
const explicitRuntime = createRuntime(explicitTarget.target);
scheduleRouteFocus({ pathname: "/onboarding", hash: "#local-data", key: "entry-b" }, explicitRuntime.runtime);
explicitRuntime.flush(explicitRuntime.state.requested[0]);
assertEqual(explicitTarget.state.tabIndexWrites.length, 0, "an explicit tabindex is preserved");

const nativeTarget = createTarget({ nativeFocusable: true });
const nativeRuntime = createRuntime(nativeTarget.target);
scheduleRouteFocus({ pathname: "/onboarding", hash: "#local-data", key: "entry-c" }, nativeRuntime.runtime);
nativeRuntime.flush(nativeRuntime.state.requested[0]);
assertEqual(nativeTarget.state.tabIndexWrites.length, 0, "a native focus target does not receive a redundant tabindex");

const missingRuntime = createRuntime(null);
scheduleRouteFocus({ pathname: "/onboarding", hash: "#missing", key: "entry-d" }, missingRuntime.runtime);
missingRuntime.flush(missingRuntime.state.requested[0]);
assertEqual(missingRuntime.state.targetIds.join(","), "missing", "a missing hash target is handled without throwing");

const cancelledTarget = createTarget();
const cancelledRuntime = createRuntime(cancelledTarget.target);
const cleanup = scheduleRouteFocus(
  { pathname: "/onboarding", hash: "#local-data", key: "entry-e" },
  cancelledRuntime.runtime,
);
cleanup?.();
cancelledRuntime.flush(cancelledRuntime.state.requested[0]);
assertEqual(cancelledRuntime.state.cancelled.join(","), "1", "cleanup cancels the pending animation frame");
assertEqual(cancelledTarget.state.focusOptions.length, 0, "a cancelled frame cannot focus a stale target");

const repeatedTarget = createTarget();
const repeatedRuntime = createRuntime(repeatedTarget.target);
const firstCleanup = scheduleRouteFocus(
  { pathname: "/onboarding", hash: "#local-data", key: "entry-f" },
  repeatedRuntime.runtime,
);
firstCleanup?.();
scheduleRouteFocus({ pathname: "/onboarding", hash: "#local-data", key: "entry-g" }, repeatedRuntime.runtime);
assertEqual(repeatedRuntime.state.requested.length, 2, "the same hash with a new location key schedules focus again");
repeatedRuntime.flush(repeatedRuntime.state.requested[1]);
assertEqual(repeatedTarget.state.focusOptions.length, 1, "the replacement navigation focuses the target");

const routeFocusSource = readFileSync(
  new URL("../src/shared/page-shell/RouteFocusManager.tsx", import.meta.url),
  "utf8",
);
assertEqual(routeFocusSource.includes("const { pathname, hash, key } = useLocation()"), true, "route focus reads the location key");
assertEqual(routeFocusSource.includes("[pathname, hash, key]"), true, "route focus reruns when the location key changes");

const navMarkup = renderToStaticMarkup(
  createElement(
    StaticRouter,
    { location: "/onboarding#local-data" },
    createElement(
      "nav",
      null,
      createElement(NavLink, { to: "/onboarding" }, "指挥台"),
      createElement(NavLink, { to: "/onboarding#local-data", "aria-current": false }, "本地数据"),
    ),
  ),
);
const ariaCurrentValues = Array.from(navMarkup.matchAll(/aria-current="([^"]+)"/g), (match) => match[1]);
assertEqual(ariaCurrentValues.join(","), "page,false", "only the primary command-center link is announced as the current page");
