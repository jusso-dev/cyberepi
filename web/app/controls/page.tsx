"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";

export default function ControlsPage() {
  const [assumption, setAssumption] = useState("");
  const [rows, setRows] = useState<Array<{ kind: string; description: string }>>([]);
  useEffect(() => {
    api.controls().then((body) => { setAssumption(body.assumption); setRows(body.controls); }).catch(() => undefined);
  }, []);
  return (
    <section className="max-w-3xl">
      <h1 className="text-4xl">Controls</h1>
      <p className="mt-2 text-muted">{assumption}. Every value can be overridden in a scenario.</p>
      <ul className="mt-8 space-y-4">
        {rows.map((row) => <li key={row.kind}><strong className="font-medium">{row.kind}</strong> — {row.description}</li>)}
      </ul>
    </section>
  );
}
