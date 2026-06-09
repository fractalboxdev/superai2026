// First-run setup (spec 10 §4): role → identity → tailscale → brain home →
// start → auto-start. Leaves the machine in a supervised running state.

import { useEffect, useState } from "react";
import logo from "@superai2026/design-system/logo.svg";
import {
  type AppState,
  detectTailscale,
  ensureIdentity,
  getAppState,
  markConfigured,
  type Role,
  saveSettings,
  setAutostart,
  startService,
  type TailscaleInfo,
} from "../ipc";

const STEPS = ["Role", "Identity", "Tailscale", "Brain", "Start", "Auto-start"];

export function Wizard({
  state,
  onDone,
}: {
  state: AppState;
  onDone: () => Promise<void>;
}) {
  const [step, setStep] = useState(0);
  const [role, setRole] = useState<Role>("host");
  const [principal, setPrincipal] = useState(state.knownPrincipals[0] ?? "cfo");
  const [relayAddr, setRelayAddr] = useState(state.settings.relayAddr);
  const [brainHome, setBrainHome] = useState("");
  const [ts, setTs] = useState<TailscaleInfo>(state.tailscale);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [keychainNote, setKeychainNote] = useState<string | null>(null);
  const [started, setStarted] = useState(false);

  // Re-poll tailscale while on that step so "install, then continue" works.
  useEffect(() => {
    if (step !== 2) return;
    const t = setInterval(() => void detectTailscale().then(setTs), 2000);
    return () => clearInterval(t);
  }, [step]);

  const act = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const next = () => setStep((s) => s + 1);

  return (
    <div className="shell">
      <header className="shell-header">
        <img src={logo} alt="" />
        <strong>Set up Contextful</strong>
      </header>
      <main className="shell-main">
        <div className="wizard-steps">
          {STEPS.map((label, i) => (
            <span
              key={label}
              title={label}
              className={`wizard-step-pip ${
                i === step
                  ? "wizard-step-pip--active"
                  : i < step
                    ? "wizard-step-pip--done"
                    : ""
              }`}
            />
          ))}
        </div>

        {error && <p className="callout callout--warn">{error}</p>}

        {step === 0 && (
          <section>
            <h2>What should this Mac do?</h2>
            <div className="role-grid">
              <button
                className={`cf-card cf-card--interactive ${role === "host" ? "cf-card--raised" : ""}`}
                onClick={() => setRole("host")}
              >
                <h3>Host</h3>
                <p>
                  Run the company brain here — the relay, the brain, and
                  scheduled ingest. One machine per company.
                </p>
              </button>
              <button
                className={`cf-card cf-card--interactive ${role === "member" ? "cf-card--raised" : ""}`}
                onClick={() => setRole("member")}
              >
                <h3>Member</h3>
                <p>
                  Keep rooms synced to local files on this Mac and stay
                  present even when the browser is closed.
                </p>
              </button>
            </div>
            <div className="actions">
              <button
                className="cf-btn cf-btn--primary"
                disabled={busy}
                onClick={() =>
                  act(async () => {
                    const addr =
                      role === "host" ? "0.0.0.0:7878" : relayAddr;
                    setRelayAddr(addr);
                    await saveSettings({ role, relayAddr: addr });
                    next();
                  })
                }
              >
                Continue
              </button>
            </div>
          </section>
        )}

        {step === 1 && (
          <section>
            <h2>Who are you?</h2>
            <p>
              Your private key is generated once and stored in the macOS
              Keychain — never written to disk in plain text.
            </p>
            <div className="field">
              <label htmlFor="principal">Principal</label>
              <input
                id="principal"
                list="known-principals"
                value={principal}
                onChange={(e) => setPrincipal(e.target.value)}
              />
              <datalist id="known-principals">
                {state.knownPrincipals.map((p) => (
                  <option key={p} value={p} />
                ))}
              </datalist>
              <span className="hint">
                e.g. <code>cfo</code> for a person, or{" "}
                <code>agent:cto/1</code> for an agent you own.
              </span>
            </div>
            {keychainNote && <p className="callout">{keychainNote}</p>}
            <div className="actions">
              <button
                className="cf-btn cf-btn--primary"
                disabled={busy || !principal}
                onClick={() =>
                  act(async () => {
                    const info = await ensureIdentity(principal, role);
                    await saveSettings({ principal });
                    setKeychainNote(
                      info.created
                        ? `Key created and stored in the Keychain (${info.keychainService}).`
                        : `Existing Keychain key found (${info.keychainService}).`,
                    );
                    next();
                  })
                }
              >
                Continue
              </button>
            </div>
          </section>
        )}

        {step === 2 && (
          <section>
            <h2>Tailscale</h2>
            <p>
              Contextful peers talk over your private tailnet. The app detects
              Tailscale but never changes its settings.
            </p>
            {!ts.installed && (
              <p className="callout callout--warn">
                Tailscale isn’t installed.{" "}
                <a href="https://tailscale.com/download/macos" target="_blank">
                  Download it
                </a>
                , sign in, then come back — this page rechecks automatically.
              </p>
            )}
            {ts.installed && !ts.running && (
              <p className="callout callout--warn">
                Tailscale is installed but not connected. Open the Tailscale
                menu-bar app and connect.
              </p>
            )}
            {ts.running && (
              <dl className="kv">
                <dt>This device</dt>
                <dd>{ts.dnsName}</dd>
                {role === "host" && ts.syncUrl && (
                  <>
                    <dt>Members connect to</dt>
                    <dd>{ts.syncUrl}</dd>
                  </>
                )}
              </dl>
            )}
            {role === "member" && (
              <div className="field">
                <label htmlFor="relay">Host relay address</label>
                <input
                  id="relay"
                  placeholder="studio.tailnet.ts.net:7878"
                  value={relayAddr}
                  onChange={(e) => setRelayAddr(e.target.value)}
                />
              </div>
            )}
            <div className="actions">
              <button
                className="cf-btn cf-btn--primary"
                disabled={busy || !ts.running || (role === "member" && !relayAddr)}
                onClick={() =>
                  act(async () => {
                    await saveSettings({ relayAddr });
                    next();
                  })
                }
              >
                Continue
              </button>
            </div>
          </section>
        )}

        {step === 3 && (
          <section>
            <h2>Brain home</h2>
            {role === "host" ? (
              <>
                <p>
                  Everything the brain knows lives in one folder on this Mac.
                </p>
                <div className="field">
                  <label htmlFor="brain">Folder</label>
                  <input
                    id="brain"
                    placeholder="~/.contextful"
                    value={brainHome}
                    onChange={(e) => setBrainHome(e.target.value)}
                  />
                  <span className="hint">Leave empty for the default.</span>
                </div>
              </>
            ) : (
              <p>
                Members don’t host the brain — rooms sync into{" "}
                <code>~/.contextful</code> automatically. Nothing to set up.
              </p>
            )}
            <div className="actions">
              <button
                className="cf-btn cf-btn--primary"
                disabled={busy}
                onClick={() =>
                  act(async () => {
                    await saveSettings({ brainHome: brainHome || null });
                    next();
                  })
                }
              >
                Continue
              </button>
            </div>
          </section>
        )}

        {step === 4 && (
          <section>
            <h2>Start</h2>
            <p>
              {role === "host"
                ? "Start the relay and brain on this Mac."
                : "Connect to the host and start syncing."}
            </p>
            {started ? (
              <StartHealth onHealthy={next} />
            ) : (
              <div className="actions">
                <button
                  className="cf-btn cf-btn--primary"
                  disabled={busy}
                  onClick={() =>
                    act(async () => {
                      await startService();
                      setStarted(true);
                    })
                  }
                >
                  Start now
                </button>
              </div>
            )}
          </section>
        )}

        {step === 5 && (
          <section>
            <h2>Start at login?</h2>
            <p>
              Recommended for the host: a small launchd agent starts Contextful
              in the menu bar whenever this Mac boots, so the brain stays
              reachable without anyone logging into a terminal.
            </p>
            <div className="actions">
              <button
                className="cf-btn cf-btn--primary"
                disabled={busy}
                onClick={() =>
                  act(async () => {
                    await setAutostart(true);
                    await markConfigured();
                    await onDone();
                  })
                }
              >
                Enable auto-start
              </button>
              <button
                className="cf-btn cf-btn--ghost"
                disabled={busy}
                onClick={() =>
                  act(async () => {
                    await markConfigured();
                    await onDone();
                  })
                }
              >
                Skip
              </button>
            </div>
          </section>
        )}
      </main>
    </div>
  );
}

/** Poll until the supervisor reports healthy, then advance. */
function StartHealth({ onHealthy }: { onHealthy: () => void }) {
  const [detail, setDetail] = useState("Waiting for the service to come up…");
  const [healthy, setHealthy] = useState(false);

  useEffect(() => {
    const t = setInterval(async () => {
      const s = await getAppState();
      setDetail(s.supervisor.detail);
      if (s.supervisor.status === "healthy") {
        setHealthy(true);
        clearInterval(t);
      }
    }, 1000);
    return () => clearInterval(t);
  }, []);

  return (
    <div>
      <p className="callout">
        {healthy ? "Up and running." : detail}
      </p>
      {healthy && (
        <div className="actions">
          <button className="cf-btn cf-btn--primary" onClick={onHealthy}>
            Continue
          </button>
        </div>
      )}
    </div>
  );
}
