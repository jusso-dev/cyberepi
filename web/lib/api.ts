export type Experiment = {
  id: string;
  name: string;
  status: string;
  runs_requested: number;
  runs_completed: number;
  summary?: Report | null;
};

export type Report = {
  population: number;
  initial_compromises: number;
  duration_seconds: number;
  pathogen: string;
  variants: Variant[];
};

export type Variant = {
  name: string;
  runs: number;
  r0: number;
  invasion_threshold: number;
  median_peak: number;
  median_attack_rate: number;
  median_outbreak: number;
  p95_outbreak: number;
  extinction_probability: number;
  median_containment_seconds: number;
  median_rt_below_seconds?: number | null;
  superspreaders: Array<{ name: string; score: number; reasons: string[]; entity_type: string }>;
  curve: Array<{ t_seconds: number; infectious: number; exposed: number; detected: number; rt: number }>;
};

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: { "content-type": "application/json", ...(init?.headers ?? {}) },
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || response.statusText);
  }
  return response.json() as Promise<T>;
}

export const api = {
  me: () => request<{ username: string }>("/api/v1/auth/me"),
  login: (username: string, password: string) =>
    request("/api/v1/auth/login", { method: "POST", body: JSON.stringify({ username, password }) }),
  experiments: () => request<Experiment[]>("/api/v1/experiments"),
  experiment: (id: string) => request<Experiment>(`/api/v1/experiments/${id}`),
  results: (id: string) => request<{ summary: Report | null; status: string; runs_completed: number; runs_requested: number }>(`/api/v1/experiments/${id}/results`),
  demo: () => request<{ id: string }>("/api/v1/demo", { method: "POST" }),
  createExperiment: (yaml: string) => request<{ id: string }>("/api/v1/experiments", { method: "POST", body: JSON.stringify({ yaml }) }),
  pathogens: () => request<{ pathogens: Array<{ name: string; description: string; illustrative: boolean }> }>("/api/v1/pathogens"),
  controls: () => request<{ assumption: string; controls: Array<{ kind: string; description: string }> }>("/api/v1/controls"),
  graph: (id: string) => request<{ nodes: GraphNode[]; edges: GraphEdge[]; note: string }>(`/api/v1/experiments/${id}/graph`),
  trace: (id: string, run = 0) => request<{ events: Array<{ t_seconds: number; kind: string; entity: number }> }>(`/api/v1/experiments/${id}/trace?run=${run}`),
};

export type GraphNode = {
  id: number;
  name: string;
  type: string;
  criticality: number;
  privilege: number;
  susceptibility: number;
  degree: number;
};

export type GraphEdge = { source: number; target: number; kind: string };
