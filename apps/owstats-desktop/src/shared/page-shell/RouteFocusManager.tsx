import { useEffect } from "react";
import { useLocation } from "react-router-dom";

type RouteFocusLocation = {
  pathname: string;
  hash: string;
  key: string;
};

type RouteFocusRuntime = {
  getElementById: (id: string) => HTMLElement | null;
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
  requestAnimationFrame: (callback) => requestAnimationFrame(callback),
  cancelAnimationFrame: (handle) => cancelAnimationFrame(handle),
};

export function scheduleRouteFocus(
  location: RouteFocusLocation,
  runtime: RouteFocusRuntime = browserRouteFocusRuntime,
) {
  if (!location.hash) {
    return;
  }

  const frame = runtime.requestAnimationFrame(() => {
    const target = runtime.getElementById(location.hash.slice(1));
    if (!target) {
      return;
    }

    if (!target.hasAttribute("tabindex") && !target.matches(nativeFocusTargetSelector)) {
      target.tabIndex = -1;
    }

    target.scrollIntoView({ block: "start" });
    target.focus({ preventScroll: true });
  });

  return () => runtime.cancelAnimationFrame(frame);
}

export function RouteFocusManager() {
  const { pathname, hash, key } = useLocation();

  useEffect(() => scheduleRouteFocus({ pathname, hash, key }), [pathname, hash, key]);

  return null;
}
