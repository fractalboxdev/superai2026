import { useEffect, useRef, useState } from "react";
import { getLogs, onSupervisorLog } from "../ipc";

export function Logs() {
  const [lines, setLines] = useState<string[]>([]);
  const ref = useRef<HTMLPreElement>(null);

  useEffect(() => {
    void getLogs(500).then(setLines);
    const sub = onSupervisorLog((line) =>
      setLines((ls) => [...ls.slice(-999), line]),
    );
    return () => {
      void sub.then((un) => un());
    };
  }, []);

  useEffect(() => {
    ref.current?.scrollTo({ top: ref.current.scrollHeight });
  }, [lines]);

  return (
    <pre className="log-view" ref={ref}>
      {lines.length ? lines.join("\n") : "No log output yet."}
    </pre>
  );
}
