-- Table to track forwarded problems
CREATE TABLE IF NOT EXISTS forwarded_problems (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    problem_id TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    severity_level TEXT,
    title TEXT NOT NULL,
    first_seen_at INTEGER NOT NULL,
    last_forwarded_at INTEGER NOT NULL,
    last_status_change_at INTEGER NOT NULL,
    forward_count INTEGER DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_problem_id ON forwarded_problems(problem_id);
CREATE INDEX IF NOT EXISTS idx_status ON forwarded_problems(status);
CREATE INDEX IF NOT EXISTS idx_last_forwarded_at ON forwarded_problems(last_forwarded_at);

-- Track forwarding history for audit
CREATE TABLE IF NOT EXISTS forward_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    problem_id TEXT NOT NULL,
    connector_name TEXT NOT NULL,
    status TEXT NOT NULL, -- 'success', 'failed', 'retrying'
    response_code INTEGER,
    error_message TEXT,
    forwarded_at INTEGER NOT NULL,
    FOREIGN KEY (problem_id) REFERENCES forwarded_problems(problem_id)
);

CREATE INDEX IF NOT EXISTS idx_forward_history_problem_id ON forward_history(problem_id);
CREATE INDEX IF NOT EXISTS idx_forward_history_connector ON forward_history(connector_name);

-- Track application state
CREATE TABLE IF NOT EXISTS app_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
