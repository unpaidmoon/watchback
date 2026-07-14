import type { ReactNode } from "react";

export type StatusPillProps = {
  tone?: "neutral" | "success" | "danger" | "warning";
  children: ReactNode;
};

export function StatusPill({ tone = "neutral", children }: StatusPillProps) {
  return (
    <span className={`statusPill statusPill--${tone}`} data-tone={tone}>
      {children}
    </span>
  );
}
