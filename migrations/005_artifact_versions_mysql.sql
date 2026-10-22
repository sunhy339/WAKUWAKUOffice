-- 产物版本表：保存不可变的产物快照，恢复操作只会生成新的当前版本。
CREATE TABLE IF NOT EXISTS artifact_versions (
    id VARCHAR(36) PRIMARY KEY,
    session_id VARCHAR(36) NOT NULL,
    artifact_id VARCHAR(36) NOT NULL,
    version BIGINT NOT NULL,
    payload LONGTEXT NOT NULL,
    created_at VARCHAR(64) NOT NULL,
    created_by VARCHAR(36) NULL,
    UNIQUE KEY uq_artifact_versions_artifact_version (session_id, artifact_id, version),
    INDEX idx_artifact_versions_lookup (session_id, artifact_id, version),
    CONSTRAINT fk_artifact_versions_session
        FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
