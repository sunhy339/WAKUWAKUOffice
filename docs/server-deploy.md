# WaKuOffice 服务器部署

## 目标

该部署方案把账户数据库、应用运行时、文件实体和对象存储放在服务器上：

- SQLite 数据库和项目元数据保存在 Docker volume `wakuoffice-data`；
- 文件实体保存在 MinIO volume `wakuoffice-minio-data`；
- 应用通过内部网络访问 MinIO，不把对象存储 API 暴露到公网；
- MinIO 控制台只绑定服务器本机 `19001`，需要管理时使用 SSH 隧道；
- 应用使用 `8000` 端口，和服务器现有 `app-src` 项目的 `18080` 端口分离。

## 配置

在项目根目录创建服务器专用 `.env.server`，不要提交到 Git：

```ini
AIPPT_JWT_SECRET=请生成随机长密钥
LLM_TEXT_BASE_URL=https://你的模型服务/v1
LLM_TEXT_API_KEY=你的模型服务密钥
LLM_TEXT_MODELS=你的模型名
LLM_TEXT_PROVIDER=OpenAI
LLM_TEXT_WIRE_API=responses
LLM_TEXT_REASONING_EFFORT=medium
LLM_IMAGE_BASE_URL=https://你的图片服务/v1
LLM_IMAGE_API_KEY=你的图片服务密钥
LLM_IMAGE_MODELS=你的图片模型
LLM_VIDEO_BASE_URL=https://你的视频服务/v1
LLM_VIDEO_API_KEY=你的视频服务密钥
LLM_VIDEO_MODELS=你的视频模型
STORAGE_S3_BUCKET=wakuoffice
STORAGE_S3_ACCESS_KEY=随机管理员用户名
STORAGE_S3_SECRET_KEY=随机长密钥
AIPPT_CORS_ORIGINS=http://服务器IP:8000
```

## 启动与检查

```bash
docker compose --env-file .env.server -f docker-compose-server.yml build walioffice
docker compose --env-file .env.server -f docker-compose-server.yml up -d minio minio-init walioffice
docker compose --env-file .env.server -f docker-compose-server.yml ps
curl http://127.0.0.1:8000/api/health
```

首次启动会自动创建私有 bucket。应用启动失败时，先查看：

```bash
docker compose --env-file .env.server -f docker-compose-server.yml logs --tail=100 minio-init walioffice
```

## 管理 MinIO

在本地建立 SSH 隧道后访问 `http://127.0.0.1:19001`：

```bash
ssh -L 19001:127.0.0.1:19001 root@服务器IP
```

不要把 `.env.server`、MinIO 密钥或应用 JWT 密钥提交到仓库。该方案使用 Docker volume 持久化数据，停止并重新创建容器不会删除数据；删除 volume 前必须先做备份和明确确认。
