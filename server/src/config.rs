use anyhow::Result;
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub app_name: String,
    pub host: String,
    pub port: u16,

    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
    pub auth_mode: String,
    pub auth_local_username: String,
    pub auth_reset_token_ttl_minutes: i64,
    pub auth_reset_base_url: String,
    pub auth_reset_dev_mode: bool,
    pub auth_reset_webhook_url: Option<String>,
    pub x_api_auth_login_url: String,

    pub llm_base_url: String,
    pub llm_api_key: String,
    pub llm_api_keys: Vec<String>,
    pub llm_model: String,
    pub llm_text_base_url: String,
    pub llm_text_api_key: String,
    pub llm_text_api_keys: Vec<String>,
    pub llm_text_model: String,
    pub llm_text_models: Vec<String>,
    pub llm_text_provider: String,
    pub llm_text_wire_api: String,
    pub llm_text_reasoning_effort: String,
    pub llm_image_base_url: String,
    pub llm_image_api_key: String,
    pub llm_image_api_keys: Vec<String>,
    pub llm_image_model: String,
    pub llm_image_models: Vec<String>,
    pub llm_video_base_url: String,
    pub llm_video_api_key: String,
    pub llm_video_api_keys: Vec<String>,
    pub llm_video_model: String,
    pub llm_video_models: Vec<String>,
    pub llm_provider: String,
    pub llm_tool_timeout_ms: u64,
    pub llm_chat_timeout_ms: u64,
    pub web_search_provider: String,
    pub web_search_endpoint: String,
    pub web_search_timeout_ms: u64,

    pub data_dir: String,
    pub projects_dir: String,
    pub sessions_dir: String,
    pub render_output_dir: String,

    // 文件存储配置：local 使用 data/files，s3 使用 S3 兼容对象存储。
    pub storage_backend: String,
    pub storage_s3_endpoint: String,
    pub storage_s3_region: String,
    pub storage_s3_bucket: String,
    pub storage_s3_access_key: String,
    pub storage_s3_secret_key: String,
    pub storage_s3_path_style: bool,

    pub cors_origins: Vec<String>,

    // 数据库配置
    pub database_url: String,
    pub db_max_connections: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let jwt_secret = env_or_required("AIPPT_JWT_SECRET");

        let llm_text_base_url = env_or_required("LLM_TEXT_BASE_URL");
        let llm_text_api_key = env_or_required("LLM_TEXT_API_KEY");
        let llm_text_api_keys = split_api_keys(&llm_text_api_key);

        let llm_image_base_url = env_or_required("LLM_IMAGE_BASE_URL");
        let llm_image_api_key = env_or_required("LLM_IMAGE_API_KEY");
        let llm_image_api_keys = split_api_keys(&llm_image_api_key);

        let llm_video_base_url = env_or_required("LLM_VIDEO_BASE_URL");
        let llm_video_api_key = env_or_required("LLM_VIDEO_API_KEY");
        let llm_video_api_keys = split_api_keys(&llm_video_api_key);

        // 模型列表（必填，逗号分隔）；默认模型取列表首个或 *_MODELS_DEFAULT
        // 向后兼容：如果没配 LLM_TEXT_MODELS 但配了 LLM_TEXT_MODEL，则用后者作为单元素列表
        let llm_text_models = match env::var("LLM_TEXT_MODELS") {
            Ok(v) if !v.trim().is_empty() => split_env_list(&v),
            _ => vec![env_or_required("LLM_TEXT_MODEL")],
        };
        let llm_text_model = {
            let d = env_or("LLM_TEXT_MODELS_DEFAULT", "");
            if d.trim().is_empty() {
                llm_text_models.first().cloned().unwrap_or_default()
            } else {
                d
            }
        };
        let llm_text_provider = env_or("LLM_TEXT_PROVIDER", "OpenAI Compatible");
        let llm_text_wire_api = match env_or("LLM_TEXT_WIRE_API", "chat_completions").as_str() {
            "responses" => "responses".to_string(),
            _ => "chat_completions".to_string(),
        };
        let llm_text_reasoning_effort = env_or("LLM_TEXT_REASONING_EFFORT", "medium");

        let llm_image_models = match env::var("LLM_IMAGE_MODELS") {
            Ok(v) if !v.trim().is_empty() => split_env_list(&v),
            _ => vec![env_or_required("LLM_IMAGE_MODEL")],
        };
        let llm_image_model = {
            let d = env_or("LLM_IMAGE_MODELS_DEFAULT", "");
            if d.trim().is_empty() {
                llm_image_models.first().cloned().unwrap_or_default()
            } else {
                d
            }
        };

        let llm_video_models = match env::var("LLM_VIDEO_MODELS") {
            Ok(v) if !v.trim().is_empty() => split_env_list(&v),
            _ => vec![env_or_required("LLM_VIDEO_MODEL")],
        };
        let llm_video_model = {
            let d = env_or("LLM_VIDEO_MODELS_DEFAULT", "");
            if d.trim().is_empty() {
                llm_video_models.first().cloned().unwrap_or_default()
            } else {
                d
            }
        };

        let llm_model = llm_text_model.clone();
        let llm_api_key = llm_text_api_key.clone();
        let llm_api_keys = llm_text_api_keys.clone();

        // 数据库配置：如果不配 DATABASE_URL，默认使用 SQLite
        let data_dir = env_or("AIPPT_DATA_DIR", "data");
        let database_url = env::var("DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| sqlite_database_url(&data_dir));

        let x_api_auth_login_url = env::var("WAKUOFFICE_X_API_AUTH_LOGIN_URL")
            .or_else(|_| env::var("WALIOFFICE_X_API_AUTH_LOGIN_URL"))
            .unwrap_or_else(|_| "https://x-api.itedus.cn/api/v1/auth/login".to_string());
        let configured_app_name = env_or("AIPPT_APP_NAME", "WaKuOffice");
        let app_name = if configured_app_name.eq_ignore_ascii_case("walioffice") {
            "WaKuOffice".to_string()
        } else {
            configured_app_name
        };

        Ok(Self {
            app_name,
            host: env_or("AIPPT_HOST", "0.0.0.0"),
            port: env_or("AIPPT_PORT", "8000").parse().unwrap_or(8000),

            jwt_secret,
            jwt_expiry_hours: env_or("AIPPT_JWT_EXPIRY_HOURS", "24").parse().unwrap_or(24),
            auth_mode: env_or("AIPPT_AUTH_MODE", "account"),
            auth_local_username: env_or("AIPPT_AUTH_LOCAL_USERNAME", "local-user"),
            auth_reset_token_ttl_minutes: env_or("AIPPT_AUTH_RESET_TOKEN_TTL_MINUTES", "20")
                .parse()
                .unwrap_or(20)
                .clamp(5, 120),
            auth_reset_base_url: env_or(
                "AIPPT_AUTH_RESET_BASE_URL",
                "http://localhost:5173/reset-password",
            ),
            auth_reset_dev_mode: env::var("AIPPT_AUTH_RESET_DEV_MODE")
                .ok()
                .and_then(|value| value.parse::<bool>().ok())
                .unwrap_or(cfg!(debug_assertions)),
            auth_reset_webhook_url: env::var("AIPPT_AUTH_RESET_WEBHOOK_URL")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            x_api_auth_login_url,

            llm_base_url: llm_text_base_url.clone(),
            llm_api_key,
            llm_api_keys,
            llm_model,
            llm_text_base_url,
            llm_text_api_key,
            llm_text_api_keys,
            llm_text_model,
            llm_text_models,
            llm_text_provider,
            llm_text_wire_api,
            llm_text_reasoning_effort,
            llm_image_base_url,
            llm_image_api_key,
            llm_image_api_keys,
            llm_image_model,
            llm_image_models,
            llm_video_base_url,
            llm_video_api_key,
            llm_video_api_keys,
            llm_video_model,
            llm_video_models,
            llm_provider: env_or("AIPPT_LLM_PROVIDER", "glm-gateway"),
            llm_tool_timeout_ms: env_or("AIPPT_LLM_TOOL_TIMEOUT_MS", "1800000")
                .parse()
                .unwrap_or(1_800_000),
            llm_chat_timeout_ms: env_or("AIPPT_LLM_CHAT_TIMEOUT_MS", "1800000")
                .parse()
                .unwrap_or(1_800_000),
            web_search_provider: env_or("AIPPT_WEB_SEARCH_PROVIDER", "auto"),
            web_search_endpoint: env_or("AIPPT_WEB_SEARCH_ENDPOINT", "http://127.0.0.1:8080"),
            web_search_timeout_ms: env_or("AIPPT_WEB_SEARCH_TIMEOUT_MS", "20000")
                .parse()
                .unwrap_or(20_000),

            data_dir: data_dir.clone(),
            projects_dir: env_or("AIPPT_PROJECTS_DIR", "data/projects"),
            sessions_dir: env_or("AIPPT_SESSIONS_DIR", "data/sessions"),
            render_output_dir: env_or("AIPPT_RENDER_OUTPUT_DIR", "outputs"),

            storage_backend: env_or("STORAGE_BACKEND", "local").to_lowercase(),
            storage_s3_endpoint: env_or("STORAGE_S3_ENDPOINT", ""),
            storage_s3_region: env_or("STORAGE_S3_REGION", "us-east-1"),
            storage_s3_bucket: env_or("STORAGE_S3_BUCKET", "wakuoffice"),
            storage_s3_access_key: env_or("STORAGE_S3_ACCESS_KEY", ""),
            storage_s3_secret_key: env_or("STORAGE_S3_SECRET_KEY", ""),
            storage_s3_path_style: env_or("STORAGE_S3_PATH_STYLE", "true")
                .parse()
                .unwrap_or(true),

            cors_origins: env_or("AIPPT_CORS_ORIGINS", "http://localhost:5173")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),

            database_url,
            db_max_connections: env_or("DB_MAX_CONNECTIONS", "8").parse().unwrap_or(8),
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in &[
            &self.data_dir,
            &self.projects_dir,
            &self.sessions_dir,
            &self.render_output_dir,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// 判断是否使用 MySQL
    pub fn is_mysql(&self) -> bool {
        self.database_url.starts_with("mysql://")
    }

    pub fn is_local_auth(&self) -> bool {
        self.auth_mode.eq_ignore_ascii_case("local")
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn split_api_keys(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn split_env_list(value: &str) -> Vec<String> {
    value
        .split(|ch| matches!(ch, ',' | ';' | '\n'))
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn sqlite_database_url(data_dir: &str) -> String {
    // SQLx parses the SQLite URL as a path after `sqlite://`. Normalize Windows
    // separators so an absolute `C:\\...` data directory also works.
    let normalized = data_dir.replace('\\', "/");
    let prefix = if normalized.starts_with('/')
        || normalized.len() > 1 && normalized.as_bytes()[1] == b':'
    {
        "sqlite:///"
    } else {
        "sqlite://"
    };
    format!(
        "{prefix}{}/walioffice.db?mode=rwc",
        normalized.trim_end_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::sqlite_database_url;

    #[test]
    fn sqlite_url_uses_forward_slashes_for_windows_paths() {
        assert_eq!(
            sqlite_database_url(r"C:\Users\demo\data"),
            "sqlite:///C:/Users/demo/data/walioffice.db?mode=rwc"
        );
    }

    #[test]
    fn sqlite_url_keeps_relative_paths_relative() {
        assert_eq!(
            sqlite_database_url("data"),
            "sqlite://data/walioffice.db?mode=rwc"
        );
    }
}

fn env_or_required(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| {
        eprintln!("缺少必要环境变量：{key}。请复制 .env.example 为 .env 并填写配置。");
        std::process::exit(1);
    })
}

use std::sync::OnceLock;
static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| Config::from_env().expect("config init failed"))
}
