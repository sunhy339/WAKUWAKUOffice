$ErrorActionPreference = 'Stop'

$packageRoot = Split-Path -Parent $PSScriptRoot
$exeCandidates = @(
  (Join-Path $packageRoot 'walioffice.exe'),
  (Join-Path $packageRoot 'server\target\release\walioffice.exe')
)
$executable = $exeCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1

if (-not $executable) {
  throw "找不到 walioffice.exe。请将本脚本放在发布包根目录，或先执行 npm run build。"
}

# 本地单人模式：只监听本机，不暴露给局域网；后端仍然使用 JWT 保护内部 API。
$env:AIPPT_HOST = '127.0.0.1'
$env:AIPPT_PORT = '8000'
$env:AIPPT_AUTH_MODE = 'local'
$env:AIPPT_AUTH_LOCAL_USERNAME = 'local-user'

$process = Start-Process `
  -FilePath $executable `
  -WorkingDirectory $packageRoot `
  -WindowStyle Minimized `
  -PassThru

$healthUrl = 'http://127.0.0.1:8000/api/health'
$ready = $false
for ($attempt = 0; $attempt -lt 40; $attempt++) {
  Start-Sleep -Milliseconds 250
  if ($process.HasExited) {
    throw "WaKuOffice 启动失败，退出码：$($process.ExitCode)。请检查安装目录中的 .env 配置。"
  }
  try {
    Invoke-WebRequest -Uri $healthUrl -UseBasicParsing -TimeoutSec 2 | Out-Null
    $ready = $true
    break
  } catch {
    # 服务尚未绑定端口，继续等待。
  }
}

if (-not $ready) {
  throw 'WaKuOffice 启动超时，请检查安装目录中的 .env、端口 8000 和日志。'
}

Start-Process 'http://127.0.0.1:8000'
Write-Host 'WaKuOffice 已启动：http://127.0.0.1:8000'
