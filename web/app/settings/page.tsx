"use client";

import { useEffect, useState } from "react";

export default function SettingsPage() {
  const [body, setBody] = useState<Record<string, string>>({});
  useEffect(() => {
    fetch("/api/v1/settings").then((r) => r.json()).then(setBody).catch(() => undefined);
  }, []);
  return (
    <section className="max-w-3xl">
      <h1 className="text-4xl">Settings</h1>
      <p className="mt-2 text-muted">Local accounts are enough. OIDC can be added later without becoming mandatory.</p>
      <dl className="mt-8 space-y-3">
        {Object.entries(body).map(([key, value]) => (
          <div key={key} className="grid grid-cols-[12rem_1fr] gap-4 border-b border-line py-2">
            <dt className="text-muted">{key}</dt>
            <dd>{String(value)}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
