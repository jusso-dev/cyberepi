import Link from "next/link";

export default function LabPage() {
  return (
    <section className="max-w-3xl">
      <p className="font-mono text-xs uppercase tracking-[0.16em] text-muted">Laboratory</p>
      <h1 className="mt-2 text-5xl">Start from a question</h1>
      <p className="mt-4 text-lg text-muted">How fast can a compromise move, which identities act as superspreaders, and which combination of controls brings Rt under 1?</p>
      <div className="mt-8 flex gap-3">
        <Link className="bg-ink px-4 py-2 text-paper" href="/scenarios/new">New experiment</Link>
        <Link className="border border-line px-4 py-2" href="/experiments">Past experiments</Link>
      </div>
    </section>
  );
}
