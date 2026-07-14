import { useEffect } from "react";
import { useLocation, useNavigationType } from "react-router-dom";

type RouteFocusLocation = {
  pathname: string;
  hash: string;
  key: string;
  navigationType?: "POP" | "PUSH" | "REPLACE";
};

type RouteFocusRuntime = {
  getElementById: (id: string) => HTMLElement | null;
  getPageHeading: () => HTMLElement | null;
  requestAnimationFrame: (callback: FrameRequestCallback) => number;
  cancelAnimationFrame: (handle: number) => void;
};

const nativeFocusTargetSelector = [
  "a[href]",
  "area[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "iframe",
  "summary",
  "audio[controls]",
  "video[controls]",
  '[contenteditable]:not([contenteditable="false"])',
].join(", ");

const browserRouteFocusRuntime: RouteFocusRuntime = {
  getElementById: (id) => document.getElementById(id),
  getPageHeading: () => document.querySelector<HTMLElement>("main h1"),
  requestAnimationFrame: (callback) => requestAnimationFrame(callback),
  cancelAnimationFrame: (handle) => cancelAnimationFrame(handle),
};

export function scheduleRouteFocus(
  location: RouteFocusLocation,
  runtime: RouteFocusRuntime = browserRouteFocusRuntime,
) {
  if (!location.hash && location.navigationType !== "PUSH") {
    return;
  }

  const frame = runtime.requestAnimationFrame(() => {
    const target = location.hash
      ? runtime.getElementById(location.hash.slice(1))
      : runtime.getPageHeading();
    if (!target) {
      return;
    }

    if (!target.hasAttribute("tabindex") && !target.matches(nativeFocusTargetSelector)) {
      target.tabIndex = -1;
    }

    if (location.hash) {
      target.scrollIntoView({ block: "start" });
    }
    target.focus({ preventScroll: true });
  });

  return () => runtime.cancelAnimationFrame(frame);
}

export function RouteFocusManager() {
  const { pathname, hash, key } = useLocation();
  const navigationType = useNavigationType();

  useEffect(
    () => scheduleRouteFocus({ pathname, hash, key, navigationType }),
    [pathname, hash, key, navigationType],
  );

  return null;
}
