"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { api, type Experiment } from "@/lib/api";

export default function ExperimentsPage() {
  const [rows, setRows] = useState<Experiment[]>([]);
  const [error, setError] = useState("");
  useEffect(() => {
    api.experiments().then(setRows).catch((err: Error) => setError(err.message));
  }, []);
  return (
    <section>
      <h1 className="text-4xl">Experiments</h1>
      <p className="mt-2 text-muted">Queued, running, completed, failed, or cancelled.</p>
      {error ? <p className="mt-6 text-signal">{error}</p> : null}
      <ul className="mt-8 divide-y divide-line border-y border-line">
        {rows.map((row) => (
          <li key={row.id} className="flex items-baseline justify-between py-4">
            <Link href={`/experiments/${row.id}`} className="text-lg">{row.name}</Link>
            <span className="font-mono text-sm text-muted">
              {row.status} · {row.runs_completed}/{row.runs_requested}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}
