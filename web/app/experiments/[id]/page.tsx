"use client";

import { useParams } from "next/navigation";
import { useEffect, useState } from "react";
import { CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { api, type GraphNode, type Report } from "@/lib/api";
import { formatCount, formatHours, formatPercent, reproductionPhase } from "@/lib/format";

export default function ExperimentPage() {
  const params = useParams<{ id: string }>();
  const [report, setReport] = useState<Report | null>(null);
  const [progress, setProgress] = useState("");
  const [nodes, setNodes] = useState<GraphNode[]>([]);
  const [selected, setSelected] = useState<GraphNode | null>(null);

  useEffect(() => {
    let stop = false;
    async function load() {
      const body = await api.results(params.id);
      if (stop) return;
      setProgress(`${body.runs_completed} / ${body.runs_requested} simulations complete`);
      setReport(body.summary);
      if (body.status === "running" || body.status === "aggregating") {
        window.setTimeout(load, 3000);
      }
    }
    load().catch((err: Error) => setProgress(err.message));
    return () => { stop = true; };
  }, [params.id]);

  useEffect(() => {
    api.graph(params.id).then((graph) => setNodes(graph.nodes)).catch(() => setNodes([]));
  }, [params.id]);

  const variant = report?.variants[0];
  const phase = variant ? reproductionPhase(variant.median_rt_below_seconds == null ? variant.r0 : 0.8) : "contracting";

  return (
    <section className="max-w-5xl">
      <p className="font-mono text-xs uppercase tracking-[0.16em] text-muted">{progress}</p>
      <h1 className="mt-2 text-4xl">{variant?.name ?? "Experiment"}</h1>
      {variant ? (
        <>
          <div className="mt-8 grid grid-cols-2 gap-6 md:grid-cols-5">
            <Metric label="Population" value={formatCount(report?.population ?? 0)} />
            <Metric label="Median outbreak" value={formatCount(variant.median_outbreak)} />
            <Metric label="R0" value={variant.r0.toFixed(2)} />
            <Metric label="Attack rate" value={formatPercent(variant.median_attack_rate)} note={phase} />
            <Metric label="Containment" value={formatHours(variant.median_containment_seconds)} />
          </div>
          <p className="mt-4 text-sm text-muted">
            Rt below 1: {formatHours(variant.median_rt_below_seconds)}. Extinction {formatPercent(variant.extinction_probability)}.
            95th percentile outbreak {formatCount(variant.p95_outbreak)}.
          </p>
          <div className="mt-8 h-72 border border-line p-3">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={variant.curve}>
                <CartesianGrid stroke="#d9d1c3" />
                <XAxis dataKey="t_seconds" hide />
                <YAxis stroke="#6d675e" />
                <Tooltip />
                <Line type="monotone" dataKey="infectious" stroke="#b8432f" dot={false} name="Compromised" />
                <Line type="monotone" dataKey="exposed" stroke="#b86a1a" dot={false} name="Exposed" />
                <Line type="monotone" dataKey="rt" stroke="#1d4e89" dot={false} name="Rt" />
              </LineChart>
            </ResponsiveContainer>
          </div>
          <h2 className="mt-10 text-2xl">Potential superspreaders</h2>
          <ol className="mt-4 space-y-3">
            {variant.superspreaders.slice(0, 8).map((node, index) => (
              <li key={node.name}>
                <span className="font-mono text-sm text-muted">{index + 1}</span> {node.name}
                <span className="text-muted"> · {node.entity_type} · {node.score.toFixed(2)}</span>
                <p className="text-sm text-muted">{node.reasons[0]}</p>
              </li>
            ))}
          </ol>
          {report && report.variants.length > 1 ? (
            <table className="mt-10 w-full text-left text-sm">
              <thead>
                <tr className="border-b border-line">
                  <th className="py-2 font-medium">Scenario</th>
                  <th>Median outbreak</th>
                  <th>R0</th>
                  <th>Peak</th>
                </tr>
              </thead>
              <tbody>
                {report.variants.map((item) => (
                  <tr key={item.name} className="border-b border-line">
                    <td className="py-2">{item.name}</td>
                    <td>{formatCount(item.median_outbreak)}</td>
                    <td>{item.r0.toFixed(2)}</td>
                    <td>{formatCount(item.median_peak)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : null}
        </>
      ) : <p className="mt-8 text-muted">Waiting for the first completed batch.</p>}
      <h2 className="mt-12 text-2xl">Contact sample</h2>
      <p className="text-sm text-muted">High-degree entities from the synthetic organisation. Shape and label carry the type; colour is not the only signal.</p>
      <ul className="mt-4 grid gap-2 md:grid-cols-2">
        {nodes.slice(0, 12).map((node) => (
          <li key={node.id}>
            <button className="w-full border border-line px-3 py-2 text-left" type="button" onClick={() => setSelected(node)}>
              <span className="font-mono text-xs text-muted">{node.type}</span>
              <span className="ml-2">{node.name}</span>
            </button>
          </li>
        ))}
      </ul>
      {selected ? (
        <aside className="mt-4 border border-ink p-4">
          <h3 className="text-xl">{selected.name}</h3>
          <p>Type {selected.type}. Degree {selected.degree}.</p>
          <p>Criticality {selected.criticality.toFixed(2)}. Privilege {selected.privilege.toFixed(2)}. Susceptibility {selected.susceptibility.toFixed(2)}.</p>
        </aside>
      ) : null}
    </section>
  );
}

function Metric({ label, value, note }: { label: string; value: string; note?: string }) {
  return (
    <div>
      <p className="text-xs uppercase tracking-[0.14em] text-muted">{label}</p>
      <p className="figure text-4xl">{value}</p>
      {note ? <p className="text-xs text-muted">{note}</p> : null}
    </div>
  );
}
