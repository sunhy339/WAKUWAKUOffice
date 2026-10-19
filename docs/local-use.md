# WaKuOffice 本地使用说明

## 产品形态

WaKuOffice 是一个 Web 应用。Windows 本地包中的 exe 负责启动 Rust 服务端，浏览器负责显示 React 界面；它不是必须安装独立桌面客户端的应用。

```text
双击 start-local.bat
        ↓
启动 walioffice.exe
        ↓
本地 SQLite + 本地文件目录
        ↓
自动打开 http://127.0.0.1:8000
```

## 普通用户使用方式

发布包建议包含：

```text
WaKuOffice/
├── walioffice.exe
├── .env
├── start-local.bat
├── start-local.ps1
└── data/                 # 首次启动自动创建
```

用户双击 `start-local.bat` 即可。启动器会：

1. 将服务绑定到 `127.0.0.1`，不向局域网开放；
2. 使用 `AIPPT_AUTH_MODE=local` 启用本地单人模式；
3. 启动后端并等待健康检查通过；
4. 自动打开默认浏览器；
5. 将 SQLite 数据和生成文件保存在包目录下的 `data/` 和 `outputs/`。

本地单人模式不显示注册登录页面，后端会为当前本机创建一个固定的内部用户 `local-user`。JWT 仍然用于保护后端 API，但用户不需要手动输入账号密码。

## `.env` 配置

发布前将仓库根目录的 `.env.example` 复制为发布包中的 `.env`，至少填写：

```env
AIPPT_JWT_SECRET=请替换为随机长字符串
LLM_TEXT_BASE_URL=你的文本模型地址
LLM_TEXT_API_KEY=你的文本模型密钥
LLM_TEXT_MODELS=你的文本模型名
LLM_IMAGE_BASE_URL=你的图片模型地址
LLM_IMAGE_API_KEY=你的图片模型密钥
LLM_IMAGE_MODELS=你的图片模型名
LLM_VIDEO_BASE_URL=你的视频模型地址
LLM_VIDEO_API_KEY=你的视频模型密钥
LLM_VIDEO_MODELS=你的视频模型名

# 本地启动器会设置以下值；显式写在 .env 中也可以
AIPPT_AUTH_MODE=local
DATABASE_URL=
```

不要把真实密钥提交到 Git。`.env` 缺失或模型配置不完整时，exe 会因为无法初始化配置而退出，这是启动失败，不是登录失败。

## 本地用户设置 API Key

启动进入工作台后，点击顶部的“设置”按钮，打开“模型服务”页面：

1. 在 `Base URL` 填写模型服务的 OpenAI 兼容接口地址，例如服务要求的 `.../v1` 地址；
2. 选择 `Provider`、`Wire API`（`Chat Completions` 或 `Responses`）和 `Reasoning`（如果服务支持）；
3. 在 `API Key 池` 中每行填写一个 Key；只使用一个 Key 时只填写一行。页面默认遮罩 Key，点击“显示 API Key”后才能编辑；
4. 点击“从 Base URL 获取模型”，再在模型列表中保留可用模型；
5. 点击“测试当前模型连接”，确认当前 Base URL、Wire API、Key 和模型可以实际返回响应；该测试不会创建聊天会话；
6. 在顶部选择当前配置和默认模型，点击“保存设置”；
7. 返回对话页发送一条消息验证配置。

如果模型服务提供 OpenAI 兼容的 `GET {Base URL}/models`，可以点击 Base URL 下方的“从 Base URL 获取模型”，系统会读取返回的模型 ID 并合并到模型列表。获取失败时仍可以手动填写；系统不会把 API Key 返回给浏览器以外的响应内容。

本地用户不需要注册、登录或手动维护账号。设置由当前本机的内部用户保存到 SQLite 的 `user_settings` 数据中，聊天请求会读取当前启用的配置。多个 Key 会按请求轮询，遇到 401、403、429 或服务端错误时会尝试池中的下一个 Key。

当前页面主要管理对话/Agent 使用的文本模型。图片、视频等专用模型仍可通过 `.env` 配置；后续可以把它们也合并到同一个模型服务管理页。当前 API Key 是本地 SQLite 配置的一部分，暂未做数据库级加密，请妥善保护 `data/` 目录。

## 文件实体的存储方式

当前版本的“我的文件”采用“数据库保存元数据、文件系统保存实体”的方式：

```text
data/walioffice.db              # 文件名、类型、大小、所属用户、文件夹等元数据
data/files/<owner_id>/<uuid>.* # 上传文件和聊天附件的实际内容
data/projects/                  # PPT 项目 JSON
outputs/                        # 导出或渲染结果
```

在线部署时可以切换为小型的 S3 兼容对象存储（优先考虑单机 MinIO），但不建议把文件二进制放进 MySQL，也不建议用 Redis 作为文件的唯一存储。推荐的拆分是：MySQL 保存文件元数据和权限关系，对象存储保存文件实体，Redis 只用于限流、短期缓存和任务状态。

对象存储适配将保持现有 `/api/files` 接口不变：后端先校验用户权限，再由后端完成上传、读取和删除；浏览器和用户不会直接拿到对象存储长期密钥。对象键按用户隔离，例如 `users/<owner_id>/files/<uuid>.<ext>`，桶保持私有。后续如果需要大文件直传，再单独增加短时签名 URL，不改变当前权限边界。

当前版本已经支持双模式：`STORAGE_BACKEND=local` 时保持原有 `data/files/` 行为；配置为 `s3` 并填写下面的连接参数后，新上传、聊天生成文件、下载、预览、缩略图和删除都会走 S3 兼容存储。数据库会保存 `storage_backend` 和 `storage_key`，同时保留 `file_path` 兼容旧本地文件，因此切换不会影响已有本地记录。

```env
STORAGE_BACKEND=s3
STORAGE_S3_ENDPOINT=http://minio:9000
STORAGE_S3_REGION=us-east-1
STORAGE_S3_BUCKET=wakuoffice
STORAGE_S3_ACCESS_KEY=你的访问密钥
STORAGE_S3_SECRET_KEY=你的私密密钥
STORAGE_S3_PATH_STYLE=true
```

这些参数应保存到服务器上的 `.env` 或部署平台的 Secret/Environment 配置中，不要提交到 Git。服务启动时会校验配置；参数不完整或后端值不正确会直接报错，避免服务运行后才发现文件无法上传。已经存在的本地文件不会在切换配置时自动搬运，需要按下面的维护命令手动执行一次迁移。

已有本地文件可以在停掉 Web 服务后执行一次迁移。先把 `STORAGE_BACKEND` 和完整的 `STORAGE_S3_*` 参数配置好，再运行：

```bash
cargo run --release -- --migrate-storage
```

迁移成功后会更新数据库中的存储位置，并默认保留原本地文件作为回滚副本；当前版本不会自动删除本地副本。

## 数据备份和卸载

账户、本地项目、会话和文件元数据默认在：

```text
data/walioffice.db
```

备份时复制整个 `data/` 和 `outputs/` 目录。卸载程序前先备份这两个目录；删除它们会删除本地数据。

## 在线模式

远程服务器或局域网多人使用时，将：

```env
AIPPT_AUTH_MODE=account
DATABASE_URL=mysql://用户名:密码@数据库地址:3306/数据库名
```

在线模式显示注册、登录和找回密码，用户数据按 `owner_id` 隔离。Redis 可在后续用于限流、验证码和短期缓存，但不作为账户数据的唯一存储。
