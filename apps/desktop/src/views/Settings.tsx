import { useState } from "react";
import {
  type AppState,
  restartService,
  saveSettings,
  setAutostart,
} from "../ipc";

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
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const dirty =
    role !== s.role ||
    relayAddr !== s.relayAddr ||
    doc !== s.doc ||
    (brainHome || null) !== s.brainHome ||
    inference !== s.inference ||
    channel !== s.updateChannel;

  const save = async () => {
    setSaving(true);
    try {
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
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div>
      <h2>Settings</h2>

      <div className="field">
        <label htmlFor="role">Role</label>
        <select
          id="role"
          value={role}
          onChange={(e) => setRole(e.target.value as typeof role)}
        >
          <option value="host">Host — run the relay and brain here</option>
          <option value="member">Member — sync rooms from a host</option>
        </select>
      </div>

      <div className="field">
        <label htmlFor="relay">
          {role === "host" ? "Listen address" : "Host relay address"}
        </label>
        <input
          id="relay"
          value={relayAddr}
          onChange={(e) => setRelayAddr(e.target.value)}
        />
        <span className="hint">
          {role === "host"
            ? "Where the relay listens. Members reach it over the tailnet."
            : "The host's tailnet address, e.g. studio.tailnet.ts.net:7878"}
        </span>
      </div>

      {role === "member" && (
        <div className="field">
          <label htmlFor="doc">Room</label>
          <input id="doc" value={doc} onChange={(e) => setDoc(e.target.value)} />
        </div>
      )}

      {role === "host" && (
        <>
          <div className="field">
            <label htmlFor="brain">Brain home</label>
            <input
              id="brain"
              placeholder="~/.contextful"
              value={brainHome}
              onChange={(e) => setBrainHome(e.target.value)}
            />
            <span className="hint">
              Where the brain, capabilities, and rooms live. Leave empty for
              the default.
            </span>
          </div>
          <div className="field">
            <label htmlFor="inference">Inference</label>
            <select
              id="inference"
              value={inference}
              onChange={(e) => setInference(e.target.value)}
            >
              <option value="stub">Offline (no LLM)</option>
              <option value="bedrock">Cloud (Vercel AI Gateway / Bedrock)</option>
              <option value="lmstudio">On-prem (LM Studio)</option>
            </select>
          </div>
        </>
      )}

      <div className="field">
        <label htmlFor="channel">Update channel</label>
        <select
          id="channel"
          value={channel}
          onChange={(e) => setChannel(e.target.value)}
        >
          <option value="stable">Stable</option>
          <option value="beta">Beta</option>
        </select>
      </div>

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
