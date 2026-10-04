"use client";

import { useEffect, useState } from "react";

export default function ClusterPage() {
  const [body, setBody] = useState<Record<string, string>>({});
  useEffect(() => {
    fetch("/api/v1/cluster").then((response) => response.json()).then(setBody).catch(() => undefined);
  }, []);
  return (
    <section className="max-w-3xl">
      <h1 className="text-4xl">Cluster</h1>
      <p className="mt-4 text-lg">The k3s cluster runs CyberEpi. The outbreak is a simulated population. A pod is not a host unless you explicitly import topology.</p>
      <pre className="mt-6 overflow-auto border border-line p-4 font-mono text-sm">{JSON.stringify(body, null, 2)}</pre>
    </section>
  );
}
