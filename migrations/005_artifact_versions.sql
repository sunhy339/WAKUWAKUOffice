-- 产物版本表：保存不可变的产物快照，恢复操作只会生成新的当前版本。
CREATE TABLE IF NOT EXISTS artifact_versions (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    artifact_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL,
    created_by TEXT,
    UNIQUE (session_id, artifact_id, version),
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_artifact_versions_lookup
    ON artifact_versions(session_id, artifact_id, version DESC);
