ALTER TABLE files ADD COLUMN storage_backend TEXT NOT NULL DEFAULT 'local';
ALTER TABLE files ADD COLUMN storage_key TEXT;
