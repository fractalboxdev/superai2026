import { useCallback, useEffect, useState } from "react";
import logo from "@superai2026/design-system/logo.svg";
import {
  type AppState,
  getAppState,
  onNavigate,
  onSupervisorStatus,
  type SupervisorSnapshot,
} from "./ipc";
import { StatusDot } from "./components";
import { Wizard } from "./views/Wizard";
import { Status } from "./views/Status";
import { Logs } from "./views/Logs";
import { Settings } from "./views/Settings";

type Route = "status" | "logs" | "settings";

export function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [route, setRoute] = useState<Route>("status");
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setState(await getAppState());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const subs = [
      onSupervisorStatus((supervisor: SupervisorSnapshot) => {
        setState((s) => (s ? { ...s, supervisor } : s));
      }),
      onNavigate((r) => {
        if (r === "status" || r === "logs" || r === "settings") setRoute(r);
      }),
    ];
    return () => {
      for (const s of subs) void s.then((un) => un());
    };
  }, [refresh]);

  if (error) {
    return (
      <div className="shell-main">
        <div className="callout callout--warn">{error}</div>
      </div>
    );
  }
  if (!state) return null;

  if (!state.settings.configured) {
    return <Wizard state={state} onDone={refresh} />;
  }

  return (
    <div className="shell">
      <header className="shell-header">
        <img src={logo} alt="" />
        <strong>Contextful</strong>
        <StatusDot
          status={state.supervisor.status}
          title={state.supervisor.detail}
        />
        <nav className="shell-tabs">
          {(["status", "logs", "settings"] as const).map((r) => (
            <button
              key={r}
              className={`cf-btn cf-btn--sm ${route === r ? "cf-btn--secondary" : "cf-btn--ghost"}`}
              onClick={() => setRoute(r)}
            >
              {r[0].toUpperCase() + r.slice(1)}
            </button>
          ))}
        </nav>
      </header>
      <main className="shell-main">
        {route === "status" && <Status state={state} onChanged={refresh} />}
        {route === "logs" && <Logs />}
        {route === "settings" && <Settings state={state} onChanged={refresh} />}
      </main>
    </div>
  );
}
