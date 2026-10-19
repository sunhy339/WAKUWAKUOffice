mod agent;
mod auth;
mod config;
mod db;
mod error;
mod file_extract;
mod image_ocr;
mod llm;
mod models;
mod render;
mod routes;
mod state;
mod storage;

use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wakuoffice=debug".into()),
        )
        .init();

    // 加载配置
    let cfg = config::config();
    cfg.ensure_dirs()?;
    storage::validate_config()?;
    let migrate_storage = std::env::args().any(|arg| arg == "--migrate-storage");

    // 初始化数据库（异步）
    let pool = state::init_db_pool().await;
    state::set_db_pool(pool);

    if migrate_storage {
        storage::migrate_local_files(&state::db_pool()).await?;
        return Ok(());
    }

    // 注册 Agent 工具
    agent::tools::register_all_tools().await;

    // 构建路由
    let app = routes::build_router();

    // 启动服务
    let addr: SocketAddr = format!("{}:{}", cfg.host, cfg.port).parse()?;
    info!("🚀 {} running at http://{}", cfg.app_name, addr);
    info!("📝 LLM: {} @ {}", cfg.llm_model, cfg.llm_base_url);
    info!("📂 Projects: {}", cfg.projects_dir);
    info!("📦 File storage: {}", cfg.storage_backend);
    if cfg.is_mysql() {
        info!("🗄️ Database: MySQL");
    } else {
        info!("🗄️ Database: SQLite");
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
