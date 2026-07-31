import { invoke } from "@tauri-apps/api/core";

type FrontendDiagnosticKind = "windowError" | "unhandledRejection";

function describeError(value: unknown): string {
  if (value instanceof Error) {
    return `${value.name}: ${value.message}`;
  }
  if (typeof value === "string") {
    return value;
  }
  return `Non-Error rejection (${typeof value})`;
}

function report(kind: FrontendDiagnosticKind, message: string): void {
  void invoke("record_frontend_diagnostic", { kind, message }).catch(() => undefined);
}

export function installFrontendDiagnostics(): () => void {
  const onError = (event: ErrorEvent) => {
    report("windowError", event.error ? describeError(event.error) : event.message);
  };
  const onUnhandledRejection = (event: PromiseRejectionEvent) => {
    report("unhandledRejection", describeError(event.reason));
  };
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onUnhandledRejection);
  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onUnhandledRejection);
  };
}
