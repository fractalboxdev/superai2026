import { useState } from "react";
import {
  type AppState,
  restartService,
  saveSettings,
  setAutostart,
} from "../ipc";
import { Field } from "../components";
import { useBusy, useFlash } from "../hooks";

export function Settings({
  state,
  onChanged,
}: {
  state: AppState;
  onChanged: () => Promise<void>;
}) {
  const s = state.settings;
  const [role, setRole] = useState(s.role);
  const [relayAddr, setRelayAddr] = useState(s.relayAddr);
  const [doc, setDoc] = useState(s.doc);
  const [brainHome, setBrainHome] = useState(s.brainHome ?? "");
  const [inference, setInference] = useState(s.inference);
  const [channel, setChannel] = useState(s.updateChannel);
  const [autostart, setAutostartState] = useState(s.autostart);
  const { busy: saving, error, run } = useBusy();
  const [saved, flashSaved] = useFlash();

  const dirty =
    role !== s.role ||
    relayAddr !== s.relayAddr ||
    doc !== s.doc ||
    (brainHome || null) !== s.brainHome ||
    inference !== s.inference ||
    channel !== s.updateChannel;

  const save = () =>
    run(async () => {
      await saveSettings({
        role,
        relayAddr,
        doc,
        brainHome: brainHome || null,
        inference,
        updateChannel: channel,
      });
      await restartService();
      await onChanged();
      flashSaved();
    });

  return (
    <div>
      <h2>Settings</h2>

      {error && <p className="callout callout--warn">{error}</p>}

      <Field id="role" label="Role">
        <select
          id="role"
          className="cf-input"
          value={role}
          onChange={(e) => setRole(e.target.value as typeof role)}
        >
          <option value="host">Host — run the relay and brain here</option>
          <option value="member">Member — sync rooms from a host</option>
        </select>
      </Field>

      <Field
        id="relay"
        label={role === "host" ? "Listen address" : "Host relay address"}
        hint={
          role === "host"
            ? "Where the relay listens. Members reach it over the tailnet."
            : "The host's tailnet address, e.g. studio.tailnet.ts.net:7878"
        }
      >
        <input
          id="relay"
          className="cf-input"
          value={relayAddr}
          onChange={(e) => setRelayAddr(e.target.value)}
        />
      </Field>

      {role === "member" && (
        <Field id="doc" label="Room">
          <input
            id="doc"
            className="cf-input"
            value={doc}
            onChange={(e) => setDoc(e.target.value)}
          />
        </Field>
      )}

      {role === "host" && (
        <>
          <Field
            id="brain"
            label="Brain home"
            hint="Where the brain, capabilities, and rooms live. Leave empty for the default."
          >
            <input
              id="brain"
              className="cf-input"
              placeholder="~/.contextful"
              value={brainHome}
              onChange={(e) => setBrainHome(e.target.value)}
            />
          </Field>
          <Field id="inference" label="Inference">
            <select
              id="inference"
              className="cf-input"
              value={inference}
              onChange={(e) => setInference(e.target.value)}
            >
              <option value="stub">Offline (no LLM)</option>
              <option value="bedrock">Cloud (Vercel AI Gateway / Bedrock)</option>
              <option value="lmstudio">On-prem (LM Studio)</option>
            </select>
          </Field>
        </>
      )}

      <Field id="channel" label="Update channel">
        <select
          id="channel"
          className="cf-input"
          value={channel}
          onChange={(e) => setChannel(e.target.value)}
        >
          <option value="stable">Stable</option>
          <option value="beta">Beta</option>
        </select>
      </Field>

      <div className="field">
        <label>
          <input
            type="checkbox"
            checked={autostart}
            onChange={async (e) => {
              const enable = e.target.checked;
              setAutostartState(enable);
              await setAutostart(enable);
              await onChanged();
            }}
          />{" "}
          Start at login (menu bar only)
        </label>
        <span className="hint">
          Installs a launchd LaunchAgent so the {role} stays up across
          reboots.
        </span>
      </div>

      <div className="actions">
        <button
          className="cf-btn cf-btn--primary"
          disabled={!dirty || saving}
          onClick={save}
        >
          {saved ? "Saved" : "Save & restart"}
        </button>
      </div>
    </div>
  );
}
