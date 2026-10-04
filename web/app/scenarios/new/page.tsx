"use client";

import { useRouter } from "next/navigation";
import { useMemo, useState } from "react";

export default function ScenarioEditorPage() {
  const router = useRouter();
  const [size, setSize] = useState(800);
  const [pathogen, setPathogen] = useState("IdentityStealer");
  const [mfa, setMfa] = useState(0);
  const [edr, setEdr] = useState(0);
  const [runs, setRuns] = useState(40);
  const [yamlMode, setYamlMode] = useState(false);
  const generated = useMemo(() => `apiVersion: cyberepi.io/v1
kind: Scenario
metadata:
  name: lab-scenario
population:
  template: medium-enterprise
  size: ${size}
  seed: 7
pathogen:
  type: ${pathogen}
initial_conditions:
  infected_entities: 1
  strategy: random
model: seidqrp
controls:
  mfa:
    coverage: ${mfa.toFixed(2)}
  edr:
    coverage: ${edr.toFixed(2)}
simulation:
  algorithm: gillespie
  duration: 7d
  runs: ${runs}
  seed: 12345
  event_mode: summary
`, [size, pathogen, mfa, edr, runs]);
  const [yaml, setYaml] = useState(generated);

  async function run() {
    const response = await fetch("/api/v1/experiments", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ yaml: yamlMode ? yaml : generated }),
    });
    if (!response.ok) throw new Error(await response.text());
    const body = await response.json() as { id: string };
    router.push(`/experiments/${body.id}`);
  }

  return (
    <section className="max-w-3xl">
      <h1 className="text-4xl">Scenario</h1>
      <p className="mt-2 text-muted">Choose an organisation, a synthetic pathogen, and the controls to compare.</p>
      <div className="mt-8 grid gap-6">
        <Slider label="Population" value={size} min={80} max={5000} step={20} onChange={setSize} />
        <label className="text-sm">Pathogen
          <select className="mt-1 block w-full border border-line bg-transparent px-3 py-2" value={pathogen} onChange={(e) => setPathogen(e.target.value)}>
            {["IdentityStealer", "RapidWorm", "EmailBorne", "CloudControlPlane", "SupplyChain", "SlowPersistent", "RansomwareLike", "InsiderLike"].map((name) => <option key={name}>{name}</option>)}
          </select>
        </label>
        <Slider label="MFA coverage" value={mfa} min={0} max={1} step={0.05} onChange={setMfa} />
        <Slider label="EDR coverage" value={edr} min={0} max={1} step={0.05} onChange={setEdr} />
        <Slider label="Monte Carlo runs" value={runs} min={1} max={500} step={1} onChange={setRuns} />
      </div>
      <button className="mt-8 text-sm underline" type="button" onClick={() => { setYamlMode((v) => !v); setYaml(generated); }}>
        {yamlMode ? "Hide advanced YAML" : "Advanced YAML"}
      </button>
      {yamlMode ? <textarea className="mt-3 h-64 w-full border border-line bg-transparent p-3 font-mono text-sm" value={yaml} onChange={(e) => setYaml(e.target.value)} /> : null}
      <button className="mt-6 bg-ink px-4 py-2 text-paper" type="button" onClick={() => run().catch((err: Error) => alert(err.message))}>Run experiment</button>
    </section>
  );
}

function Slider({ label, value, min, max, step, onChange }: { label: string; value: number; min: number; max: number; step: number; onChange: (value: number) => void }) {
  return (
    <label className="text-sm">
      <span className="flex justify-between"><span>{label}</span><span className="font-mono">{value}</span></span>
      <input className="mt-2 w-full" type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} />
    </label>
  );
}
