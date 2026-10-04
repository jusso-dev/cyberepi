"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { api } from "@/lib/api";

export default function WelcomePage() {
  const router = useRouter();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [authed, setAuthed] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    api.me().then(() => setAuthed(true)).catch(() => setAuthed(false));
  }, []);

  async function login(event: React.FormEvent) {
    event.preventDefault();
    setError("");
    try {
      await api.login(username, password);
      setAuthed(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Login failed");
    }
  }

  async function demo() {
    const created = await api.demo();
    router.push(`/experiments/${created.id}`);
  }

  return (
    <section className="max-w-3xl">
      <p className="font-mono text-xs uppercase tracking-[0.18em] text-muted">Welcome</p>
      <h1 className="mt-3 text-5xl leading-[0.95]">Welcome to CyberEpi</h1>
      <p className="mt-4 max-w-xl text-lg text-muted">
        A laboratory for asking how compromise moves through a synthetic organisation, and which
        controls push the reproduction number below one.
      </p>
      {authed ? (
        <div className="mt-10 flex flex-wrap gap-3">
          <button className="bg-ink px-4 py-2 text-paper" onClick={demo} type="button">
            Run demo experiment
          </button>
          <a className="border border-line px-4 py-2" href="/scenarios/new">Create scenario</a>
          <a className="border border-line px-4 py-2" href="/docs">Explore documentation</a>
        </div>
      ) : (
        <form className="mt-10 grid max-w-sm gap-3" onSubmit={login}>
          <label className="text-sm">
            Name
            <input className="mt-1 w-full border border-line bg-transparent px-3 py-2" value={username} onChange={(e) => setUsername(e.target.value)} />
          </label>
          <label className="text-sm">
            Password
            <input className="mt-1 w-full border border-line bg-transparent px-3 py-2" type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
          </label>
          <button className="bg-ink px-4 py-2 text-paper" type="submit">Enter laboratory</button>
          {error ? <p className="text-sm text-signal">{error}</p> : null}
        </form>
      )}
    </section>
  );
}
