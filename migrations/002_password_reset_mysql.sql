-- 密码找回 Token。只保存摘要，原始 Token 只存在于邮件链接或本地开发响应中。
CREATE TABLE IF NOT EXISTS password_reset_tokens (
    id VARCHAR(36) PRIMARY KEY,
    user_id VARCHAR(36) NOT NULL,
    token_hash VARCHAR(128) UNIQUE NOT NULL,
    expires_at VARCHAR(50) NOT NULL,
    used_at VARCHAR(50),
    created_at VARCHAR(50) NOT NULL,
    INDEX idx_password_reset_tokens_user (user_id),
    INDEX idx_password_reset_tokens_created (created_at),
    CONSTRAINT fk_password_reset_tokens_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
