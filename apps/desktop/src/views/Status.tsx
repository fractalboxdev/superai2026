import { useState } from "react";
import {
  type AppState,
  copySyncUrl,
  openWebApp,
  restartService,
  revealBrain,
  startService,
  stopService,
} from "../ipc";

const STATUS_LABEL: Record<string, string> = {
  starting: "Starting…",
  healthy: "Running",
  degraded: "Running, with issues",
  stopped: "Stopped",
};

export function Status({
  state,
  onChanged,
}: {
  state: AppState;
  onChanged: () => Promise<void>;
}) {
  const { supervisor, settings, tailscale } = state;
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);

  const run = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await fn();
      await onChanged();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <h2>
        <span className={`status-dot status-dot--${supervisor.status}`} />{" "}
        {STATUS_LABEL[supervisor.status] ?? supervisor.status}
      </h2>
      <p className="hint">{supervisor.detail}</p>

      <dl className="kv">
        <dt>Role</dt>
        <dd>{settings.role === "host" ? "Host (relay + brain)" : "Member"}</dd>
        <dt>Principal</dt>
        <dd>{settings.principal}</dd>
        <dt>{settings.role === "host" ? "Listening on" : "Relay"}</dt>
        <dd>{settings.relayAddr}</dd>
        {settings.role === "member" && (
          <>
            <dt>Room</dt>
            <dd>{settings.doc}</dd>
          </>
        )}
        <dt>Tailnet</dt>
        <dd>
          {tailscale.running
            ? (tailscale.dnsName ?? "running")
            : tailscale.installed
              ? "offline"
              : "not installed"}
        </dd>
        {tailscale.syncUrl && (
          <>
            <dt>Sync URL</dt>
            <dd>{tailscale.syncUrl}</dd>
          </>
        )}
        <dt>Restarts</dt>
        <dd>{supervisor.restarts}</dd>
        {supervisor.pid != null && (
          <>
            <dt>PID</dt>
            <dd>{supervisor.pid}</dd>
          </>
        )}
      </dl>

      {!tailscale.running && (
        <p className="callout callout--warn">
          The tailnet is offline — peers and browsers on other machines can’t
          reach this {settings.role}. Open Tailscale to reconnect.
        </p>
      )}

      <div className="actions">
        {supervisor.status === "stopped" ? (
          <button
            className="cf-btn cf-btn--primary"
            disabled={busy}
            onClick={() => run(startService)}
          >
            Start
          </button>
        ) : (
          <>
            <button
              className="cf-btn cf-btn--secondary"
              disabled={busy}
              onClick={() => run(stopService)}
            >
              Stop
            </button>
            <button
              className="cf-btn cf-btn--secondary"
              disabled={busy}
              onClick={() => run(restartService)}
            >
              Restart
            </button>
          </>
        )}
        <button className="cf-btn cf-btn--ghost" onClick={() => openWebApp()}>
          Open web app
        </button>
        {settings.role === "host" && (
          <>
            <button
              className="cf-btn cf-btn--ghost"
              onClick={() => revealBrain()}
            >
              Reveal brain in Finder
            </button>
            <button
              className="cf-btn cf-btn--ghost"
              disabled={!tailscale.syncUrl}
              onClick={async () => {
                await copySyncUrl();
                setCopied(true);
                setTimeout(() => setCopied(false), 1500);
              }}
            >
              {copied ? "Copied" : "Copy sync URL"}
            </button>
          </>
        )}
      </div>
    </div>
  );
}
