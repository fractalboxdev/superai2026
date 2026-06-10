import {
  type AppState,
  copySyncUrl,
  openWebApp,
  restartService,
  revealBrain,
  startService,
  stopService,
} from "../ipc";
import { STATUS_LABEL, StatusDot } from "../components";
import { useBusy, useFlash } from "../hooks";

export function Status({
  state,
  onChanged,
}: {
  state: AppState;
  onChanged: () => Promise<void>;
}) {
  const { supervisor, settings, tailscale } = state;
  const { busy, error, run } = useBusy();
  const [copied, flashCopied] = useFlash();

  const act = (fn: () => Promise<unknown>) =>
    run(async () => {
      await fn();
      await onChanged();
    });

  return (
    <div>
      <h2>
        <StatusDot status={supervisor.status} /> {STATUS_LABEL[supervisor.status]}
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

      {error && <p className="callout callout--warn">{error}</p>}

      <div className="actions">
        {supervisor.status === "stopped" ? (
          <button
            className="cf-btn cf-btn--primary"
            disabled={busy}
            onClick={() => act(startService)}
          >
            Start
          </button>
        ) : (
          <>
            <button
              className="cf-btn cf-btn--secondary"
              disabled={busy}
              onClick={() => act(stopService)}
            >
              Stop
            </button>
            <button
              className="cf-btn cf-btn--secondary"
              disabled={busy}
              onClick={() => act(restartService)}
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
                flashCopied();
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
