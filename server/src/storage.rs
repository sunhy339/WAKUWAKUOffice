//! 文件实体存储适配层。
//!
//! `local` 模式保留原有 data/files 目录；`s3` 模式使用 S3 兼容服务（MinIO、S3、R2 等）。
//! 业务层只处理稳定的 object key，不直接依赖具体存储 SDK。

use anyhow::{anyhow, Result};
use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::region::Region;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, OnceLock};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct StoredObject {
    pub backend: String,
    pub key: String,
    /// 本地后端保留绝对路径，方便兼容现有 file_path；S3 后端不暴露本地路径。
    pub local_path: Option<String>,
}

enum Storage {
    Local { root: PathBuf },
    S3 { bucket: Arc<Bucket> },
}

static STORAGE: OnceLock<Result<Storage, String>> = OnceLock::new();

/// 文件读取失败的稳定分类，供接口层区分 404 与存储依赖故障。
#[derive(Debug, Error)]
pub enum StorageReadError {
    #[error("文件实体不存在")]
    NotFound,
    #[error("对象存储读取失败: {0}")]
    Backend(#[source] anyhow::Error),
}

impl From<std::io::Error> for StorageReadError {
    fn from(error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::NotFound {
            Self::NotFound
        } else {
            Self::Backend(error.into())
        }
    }
}

fn classify_s3_read_error(error: s3::error::S3Error) -> StorageReadError {
    match error {
        s3::error::S3Error::HttpFailWithBody(404, _) => StorageReadError::NotFound,
        error => StorageReadError::Backend(anyhow!(error)),
    }
}

fn storage() -> Result<&'static Storage> {
    STORAGE
        .get_or_init(|| Storage::from_config().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| anyhow!(error.clone()))
}

impl Storage {
    fn from_config() -> Result<Self> {
        let cfg = crate::config::config();
        match cfg.storage_backend.as_str() {
            "local" => Ok(Self::Local {
                root: PathBuf::from(&cfg.data_dir).join("files"),
            }),
            "s3" => {
                if cfg.storage_s3_endpoint.trim().is_empty()
                    || cfg.storage_s3_access_key.trim().is_empty()
                    || cfg.storage_s3_secret_key.trim().is_empty()
                    || cfg.storage_s3_bucket.trim().is_empty()
                {
                    return Err(anyhow!(
                        "STORAGE_BACKEND=s3 时必须配置 STORAGE_S3_ENDPOINT、STORAGE_S3_BUCKET、STORAGE_S3_ACCESS_KEY 和 STORAGE_S3_SECRET_KEY"
                    ));
                }

                let region = Region::Custom {
                    region: cfg.storage_s3_region.clone(),
                    endpoint: cfg.storage_s3_endpoint.trim_end_matches('/').to_string(),
                };
                let credentials = Credentials::new(
                    Some(cfg.storage_s3_access_key.as_str()),
                    Some(cfg.storage_s3_secret_key.as_str()),
                    None,
                    None,
                    None,
                )
                .map_err(|error| anyhow!("初始化对象存储凭据失败: {error}"))?;
                let bucket = Bucket::new(&cfg.storage_s3_bucket, region, credentials)
                    .map_err(|error| anyhow!("初始化对象存储客户端失败: {error}"))?;
                let bucket = if cfg.storage_s3_path_style {
                    bucket.with_path_style()
                } else {
                    bucket
                };

                Ok(Self::S3 {
                    bucket: Arc::from(bucket),
                })
            }
            other => Err(anyhow!(
                "不支持的 STORAGE_BACKEND={other}，可选值为 local 或 s3"
            )),
        }
    }
}

/// 生成不会暴露原始文件名的对象键，并将用户隔离在自己的前缀下。
pub fn new_key(owner_id: &str, file_name: &str) -> String {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    format!(
        "users/{owner_id}/files/{}{}",
        uuid::Uuid::new_v4(),
        extension
    )
}

fn validate_key(key: &str) -> Result<()> {
    if key.trim().is_empty() {
        return Err(anyhow!("文件存储键不能为空"));
    }
    let path = Path::new(key);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(anyhow!("文件存储键不合法"));
    }
    Ok(())
}

pub async fn put(key: &str, data: &[u8], content_type: &str) -> Result<StoredObject> {
    validate_key(key)?;
    match storage()? {
        Storage::Local { root } => {
            let path = root.join(key.replace('/', std::path::MAIN_SEPARATOR_STR));
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&path, data).await?;
            Ok(StoredObject {
                backend: "local".to_string(),
                key: key.to_string(),
                local_path: Some(path.to_string_lossy().to_string()),
            })
        }
        Storage::S3 { bucket } => {
            bucket
                .put_object_with_content_type(key, data, content_type)
                .await
                .map_err(|error| anyhow!("对象存储上传失败: {error}"))?;
            Ok(StoredObject {
                backend: "s3".to_string(),
                key: key.to_string(),
                local_path: None,
            })
        }
    }
}

pub async fn get(
    backend: &str,
    key: Option<&str>,
    local_path: &str,
) -> std::result::Result<Vec<u8>, StorageReadError> {
    match backend {
        "s3" => {
            let key = key.ok_or_else(|| {
                StorageReadError::Backend(anyhow!("对象存储文件缺少 storage_key"))
            })?;
            validate_key(key).map_err(StorageReadError::Backend)?;
            match storage().map_err(StorageReadError::Backend)? {
                Storage::S3 { bucket } => bucket
                    .get_object(key)
                    .await
                    .map(|response| response.to_vec())
                    .map_err(classify_s3_read_error),
                Storage::Local { .. } => Err(StorageReadError::Backend(anyhow!(
                    "文件记录使用 S3，但当前服务未启用 S3 存储"
                ))),
            }
        }
        "local" => {
            let path = if let Some(key) = key {
                validate_key(key).map_err(StorageReadError::Backend)?;
                match storage().map_err(StorageReadError::Backend)? {
                    Storage::Local { root } => {
                        root.join(key.replace('/', std::path::MAIN_SEPARATOR_STR))
                    }
                    Storage::S3 { .. } => PathBuf::from(local_path),
                }
            } else {
                PathBuf::from(local_path)
            };
            tokio::fs::read(path).await.map_err(StorageReadError::from)
        }
        other => Err(StorageReadError::Backend(anyhow!(
            "不支持的文件存储后端: {other}"
        ))),
    }
}

pub async fn delete(backend: &str, key: Option<&str>, local_path: &str) -> Result<()> {
    match backend {
        "s3" => {
            let key = key.ok_or_else(|| anyhow!("对象存储文件缺少 storage_key"))?;
            validate_key(key)?;
            match storage()? {
                Storage::S3 { bucket } => {
                    bucket
                        .delete_object(key)
                        .await
                        .map_err(|error| anyhow!("对象存储删除失败: {error}"))?;
                    Ok(())
                }
                Storage::Local { .. } => Err(anyhow!("文件记录使用 S3，但当前服务未启用 S3 存储")),
            }
        }
        "local" => {
            let path = if let Some(key) = key {
                validate_key(key)?;
                match storage()? {
                    Storage::Local { root } => {
                        root.join(key.replace('/', std::path::MAIN_SEPARATOR_STR))
                    }
                    Storage::S3 { .. } => PathBuf::from(local_path),
                }
            } else {
                PathBuf::from(local_path)
            };
            match tokio::fs::remove_file(path).await {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.into()),
            }
        }
        other => Err(anyhow!("不支持的文件存储后端: {other}")),
    }
}

/// 将现有本地文件复制到当前配置的 S3 兼容存储，并更新文件记录。
///
/// 该操作只在 `STORAGE_BACKEND=s3` 下执行，默认保留本地副本，便于回滚和人工核验。
pub async fn migrate_local_files(pool: &crate::db::DbPool) -> Result<()> {
    let cfg = crate::config::config();
    if cfg.storage_backend != "s3" {
        return Err(anyhow!(
            "存储迁移要求 STORAGE_BACKEND=s3，当前为 {}",
            cfg.storage_backend
        ));
    }

    let files = crate::db::file_repo::list_all_files_for_storage_migration(pool).await?;
    let mut migrated = 0usize;
    let mut skipped = 0usize;

    for file in files {
        if file.storage_backend == "s3" {
            skipped += 1;
            continue;
        }

        let data = get("local", file.storage_key.as_deref(), &file.file_path)
            .await
            .map_err(|error| anyhow!("读取本地文件 {} 失败: {error}", file.id))?;
        let key = new_key(&file.owner_id, &file.name);
        let stored = match put(
            &key,
            &data,
            &mime_guess::from_path(&file.name)
                .first_or_octet_stream()
                .to_string(),
        )
        .await
        {
            Ok(stored) => stored,
            Err(error) => {
                return Err(anyhow!("上传文件 {} 到对象存储失败: {error}", file.id));
            }
        };

        if let Err(error) = crate::db::file_repo::update_storage_location(
            pool,
            &file.id,
            &stored.backend,
            &stored.key,
        )
        .await
        {
            let _ = delete("s3", Some(&stored.key), "").await;
            return Err(anyhow!("更新文件 {} 存储记录失败: {error}", file.id));
        }

        migrated += 1;
        tracing::info!(file_id = %file.id, storage_key = %stored.key, "文件已迁移到对象存储");
    }

    tracing::info!(migrated, skipped, "对象存储迁移完成；本地副本已保留");
    Ok(())
}

pub fn validate_config() -> Result<()> {
    let cfg = crate::config::config();
    match cfg.storage_backend.as_str() {
        "local" => Ok(()),
        "s3" if !cfg.storage_s3_endpoint.trim().is_empty()
            && !cfg.storage_s3_bucket.trim().is_empty()
            && !cfg.storage_s3_access_key.trim().is_empty()
            && !cfg.storage_s3_secret_key.trim().is_empty() =>
        {
            Ok(())
        }
        "s3" => Err(anyhow!("STORAGE_BACKEND=s3 时必须完整配置对象存储连接参数")),
        other => Err(anyhow!(
            "不支持的 STORAGE_BACKEND={other}，可选值为 local 或 s3"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_s3_read_error, new_key, validate_key, StorageReadError};

    #[test]
    fn object_key_is_user_scoped_and_keeps_extension() {
        let key = new_key("user-1", "报告.docx");
        assert!(key.starts_with("users/user-1/files/"));
        assert!(key.ends_with(".docx"));
    }

    #[test]
    fn object_key_rejects_path_traversal() {
        assert!(validate_key("users/user-1/../other").is_err());
        assert!(validate_key("/absolute/path").is_err());
    }

    #[test]
    fn missing_s3_object_is_not_found() {
        assert!(matches!(
            classify_s3_read_error(s3::error::S3Error::HttpFailWithBody(
                404,
                "missing".to_string()
            )),
            StorageReadError::NotFound
        ));
    }

    #[test]
    fn unavailable_s3_backend_is_not_misreported_as_not_found() {
        assert!(matches!(
            classify_s3_read_error(s3::error::S3Error::HttpFailWithBody(
                503,
                "unavailable".to_string()
            )),
            StorageReadError::Backend(_)
        ));
    }
}
