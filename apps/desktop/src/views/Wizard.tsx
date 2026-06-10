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
import { Field } from "../components";
import { useBusy } from "../hooks";

const STEPS = ["Role", "Identity", "Tailscale", "Brain", "Start", "Auto-start"];

function StepActions({
  busy,
  disabled,
  label = "Continue",
  onClick,
  secondary,
}: {
  busy: boolean;
  disabled?: boolean;
  label?: string;
  onClick: () => void;
  secondary?: { label: string; onClick: () => void };
}) {
  return (
    <div className="actions">
      <button
        className="cf-btn cf-btn--primary"
        disabled={busy || disabled}
        onClick={onClick}
      >
        {label}
      </button>
      {secondary && (
        <button
          className="cf-btn cf-btn--ghost"
          disabled={busy}
          onClick={secondary.onClick}
        >
          {secondary.label}
        </button>
      )}
    </div>
  );
}

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
  const { busy, error, run } = useBusy();
  const [keychainNote, setKeychainNote] = useState<string | null>(null);
  const [started, setStarted] = useState(false);

  // Re-poll tailscale while on that step so "install, then continue" works.
  useEffect(() => {
    if (step !== 2) return;
    const t = setInterval(() => void detectTailscale().then(setTs), 2000);
    return () => clearInterval(t);
  }, [step]);

  const next = () => setStep((s) => s + 1);

  const continueWith = (fn: () => Promise<void>) =>
    run(async () => {
      await fn();
      next();
    });

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
                className={`cf-card cf-card--interactive ${role === "host" ? "cf-card--selected" : ""}`}
                aria-pressed={role === "host"}
                onClick={() => setRole("host")}
              >
                <h3>Host</h3>
                <p>
                  Run the company brain here — the relay, the brain, and
                  scheduled ingest. One machine per company.
                </p>
              </button>
              <button
                className={`cf-card cf-card--interactive ${role === "member" ? "cf-card--selected" : ""}`}
                aria-pressed={role === "member"}
                onClick={() => setRole("member")}
              >
                <h3>Member</h3>
                <p>
                  Keep rooms synced to local files on this Mac and stay
                  present even when the browser is closed.
                </p>
              </button>
            </div>
            <StepActions
              busy={busy}
              onClick={() =>
                continueWith(async () => {
                  const addr =
                    role === "host" ? state.settings.relayAddr : relayAddr;
                  setRelayAddr(addr);
                  await saveSettings({ role, relayAddr: addr });
                })
              }
            />
          </section>
        )}

        {step === 1 && (
          <section>
            <h2>Who are you?</h2>
            <p>
              Your private key is generated once and stored in the macOS
              Keychain — never written to disk in plain text.
            </p>
            <Field
              id="principal"
              label="Principal"
              hint={
                <>
                  e.g. <code>cfo</code> for a person, or{" "}
                  <code>agent:cto/1</code> for an agent you own.
                </>
              }
            >
              <input
                id="principal"
                className="cf-input"
                list="known-principals"
                value={principal}
                onChange={(e) => setPrincipal(e.target.value)}
              />
              <datalist id="known-principals">
                {state.knownPrincipals.map((p) => (
                  <option key={p} value={p} />
                ))}
              </datalist>
            </Field>
            {keychainNote && <p className="callout">{keychainNote}</p>}
            <StepActions
              busy={busy}
              disabled={!principal}
              onClick={() =>
                continueWith(async () => {
                  const info = await ensureIdentity(principal, role);
                  await saveSettings({ principal });
                  setKeychainNote(
                    info.created
                      ? `Key created and stored in the Keychain (${info.keychainService}).`
                      : `Existing Keychain key found (${info.keychainService}).`,
                  );
                })
              }
            />
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
              <Field id="relay" label="Host relay address">
                <input
                  id="relay"
                  className="cf-input"
                  placeholder="studio.tailnet.ts.net:7878"
                  value={relayAddr}
                  onChange={(e) => setRelayAddr(e.target.value)}
                />
              </Field>
            )}
            <StepActions
              busy={busy}
              disabled={!ts.running || (role === "member" && !relayAddr)}
              onClick={() =>
                continueWith(async () => {
                  await saveSettings({ relayAddr });
                })
              }
            />
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
                <Field
                  id="brain"
                  label="Folder"
                  hint="Leave empty for the default."
                >
                  <input
                    id="brain"
                    className="cf-input"
                    placeholder="~/.contextful"
                    value={brainHome}
                    onChange={(e) => setBrainHome(e.target.value)}
                  />
                </Field>
              </>
            ) : (
              <p>
                Members don’t host the brain — rooms sync into{" "}
                <code>~/.contextful</code> automatically. Nothing to set up.
              </p>
            )}
            <StepActions
              busy={busy}
              onClick={() =>
                continueWith(async () => {
                  await saveSettings({ brainHome: brainHome || null });
                })
              }
            />
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
              <StepActions
                busy={busy}
                label="Start now"
                onClick={() =>
                  run(async () => {
                    await startService();
                    setStarted(true);
                  })
                }
              />
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
            <StepActions
              busy={busy}
              label="Enable auto-start"
              onClick={() =>
                run(async () => {
                  await setAutostart(true);
                  await markConfigured();
                  await onDone();
                })
              }
              secondary={{
                label: "Skip",
                onClick: () =>
                  run(async () => {
                    await markConfigured();
                    await onDone();
                  }),
              }}
            />
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
      {healthy && <StepActions busy={false} onClick={onHealthy} />}
    </div>
  );
}
