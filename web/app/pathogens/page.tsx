"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";

export default function PathogensPage() {
  const [rows, setRows] = useState<Array<{ name: string; description: string; illustrative: boolean }>>([]);
  useEffect(() => { api.pathogens().then((body) => setRows(body.pathogens)).catch(() => setRows([])); }, []);
  return (
    <section className="max-w-3xl">
      <h1 className="text-4xl">Synthetic pathogens</h1>
      <p className="mt-2 text-muted">Behavioural profiles only. They do not contain exploit procedures.</p>
      <ul className="mt-8 space-y-6">
        {rows.map((row) => (
          <li key={row.name}>
            <h2 className="text-2xl">{row.name}</h2>
            <p className="text-muted">{row.description}</p>
          </li>
        ))}
      </ul>
    </section>
  );
}
