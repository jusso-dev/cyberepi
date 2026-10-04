CREATE TABLE users (
    id UUID PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE scenarios (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    yaml TEXT NOT NULL,
    spec JSONB NOT NULL,
    content_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE organisations (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    template TEXT NOT NULL,
    size INTEGER NOT NULL,
    seed BIGINT NOT NULL,
    summary JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE pathogens (
    id TEXT PRIMARY KEY,
    definition JSONB NOT NULL,
    builtin BOOLEAN NOT NULL DEFAULT true
);

CREATE TABLE controls (
    id TEXT PRIMARY KEY,
    definition JSONB NOT NULL,
    assumption TEXT NOT NULL
);

CREATE TABLE experiments (
    id UUID PRIMARY KEY,
    scenario_id UUID REFERENCES scenarios(id),
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    runs_requested INTEGER NOT NULL,
    runs_completed INTEGER NOT NULL DEFAULT 0,
    seed BIGINT NOT NULL,
    algorithm TEXT NOT NULL,
    scenario_hash TEXT NOT NULL,
    scenario_yaml TEXT NOT NULL,
    cyberepi_version TEXT NOT NULL,
    generator_version TEXT NOT NULL,
    pathogen JSONB NOT NULL,
    controls JSONB NOT NULL,
    summary JSONB,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ
);

CREATE TABLE experiment_variants (
    id UUID PRIMARY KEY,
    experiment_id UUID NOT NULL REFERENCES experiments(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    controls JSONB NOT NULL,
    summary JSONB
);

CREATE TABLE simulation_batches (
    id UUID PRIMARY KEY,
    experiment_id UUID NOT NULL REFERENCES experiments(id) ON DELETE CASCADE,
    variant_name TEXT NOT NULL,
    run_offset INTEGER NOT NULL,
    run_count INTEGER NOT NULL,
    status TEXT NOT NULL,
    result JSONB,
    worker_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (experiment_id, variant_name, run_offset)
);

CREATE TABLE simulation_runs (
    id UUID PRIMARY KEY,
    experiment_id UUID NOT NULL REFERENCES experiments(id) ON DELETE CASCADE,
    variant_name TEXT NOT NULL,
    run_index INTEGER NOT NULL,
    seed BIGINT NOT NULL,
    status TEXT NOT NULL,
    summary JSONB NOT NULL,
    UNIQUE (experiment_id, variant_name, run_index)
);

CREATE INDEX simulation_runs_experiment_idx ON simulation_runs (experiment_id);
CREATE INDEX simulation_batches_experiment_idx ON simulation_batches (experiment_id);
CREATE INDEX experiments_status_idx ON experiments (status);
