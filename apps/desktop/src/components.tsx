// Small shared presentational pieces for the desktop shell.

import type { ReactNode } from "react";
import { type SupervisorStatus } from "./ipc";

export const STATUS_LABEL: Record<SupervisorStatus, string> = {
  starting: "Starting…",
  healthy: "Running",
  degraded: "Running, with issues",
  stopped: "Stopped",
};

export function StatusDot({
  status,
  title,
}: {
  status: SupervisorStatus;
  title?: string;
}) {
  return <span className={`status-dot status-dot--${status}`} title={title} />;
}

export function Field({
  id,
  label,
  hint,
  children,
}: {
  id: string;
  label: ReactNode;
  hint?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      {children}
      {hint && <span className="hint">{hint}</span>}
    </div>
  );
}
