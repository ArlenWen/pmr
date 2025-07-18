# PMR - Process Manager in Rust

PMR (Process Manager in Rust) 是一个用 Rust 编写的命令行进程管理工具，支持系统级进程管理、独立环境变量、日志管理和并发操作。

## 特性

- **系统级进程管理**: 进程独立于工具运行，不会因为工具退出而终止
- **隔离的环境变量**: 每个进程都有独立的环境变量配置
- **完整的进程操作**: 支持启动、停止、重启、删除进程
- **日志管理**: 自动捕获和管理进程的 stdout 和 stderr
- **SQLite 存储**: 使用 SQLite 数据库存储进程信息，支持并发操作
- **状态监控**: 实时监控进程状态
- **HTTP API**: 可选的 HTTP API 支持，提供 RESTful 接口进行远程管理
- **日志轮转**: 支持自动和手动日志轮转
- **(TODO)定时任务**: 支持定时执行任务
- **(TODO)进程组管理**: 支持进程组管理，支持启动、停止、重启、删除进程组

## 安装

```bash
# 克隆仓库
git clone https://github.com/ArlenWen/pmr.git
cd pmr

# 编译（基本功能）
cargo build --release

# 编译（包含 HTTP API 功能）
cargo build --release --features http-api

# 可选：安装到系统路径
cargo install --path .

# 安装包含 HTTP API 功能的版本
cargo install --path . --features http-api
```

## 使用方法

### 启动进程

```bash
# 基本用法
pmr start [选项] <进程名> <命令> [参数...]

# 示例：启动一个简单的进程
pmr start my-sleep sleep 60

# 使用环境变量
pmr start -e PORT=8080 -e DEBUG=true web-server python3 server.py

# 指定工作目录
pmr start -w /path/to/workdir my-app ./app.sh

# 指定自定义日志目录
pmr start --log-dir /var/log/myapp my-app ./app.sh

# 复杂示例
pmr start -e NGINX_PORT=80 -w /etc/nginx --log-dir /var/log/nginx nginx nginx

# 带有参数的命令（参数包含 - 或 --）
pmr start web-server python3 -m http.server --bind 127.0.0.1 8080
pmr start file-list ls -la --color=auto

# 如果需要传递 --help 参数，使用 -- 分隔符
pmr start help-cmd -- curl --help
```

### 查看进程列表

```bash
pmr list
```

输出示例：
```
NAME                 STATUS     PID        COMMAND                        CREATED             
------------------------------------------------------------------------------------------
web-server           running    12345      python3 server.py             2025-06-27 10:30:15
my-sleep             stopped    12340      sleep 60                       2025-06-27 10:25:10
```

### 查看进程状态

```bash
pmr status <进程名>
```

输出示例：
```
Process: web-server
Status: running
PID: 12345
Command: python3 server.py
Working Directory: /home/user/project
Created: 2025-06-27 10:30:15
Updated: 2025-06-27 10:30:15
Log File: /home/user/.pmr/logs/web-server.log
Environment Variables:
  PORT=8080
  DEBUG=true
```

### 查看进程日志

```bash
# 查看进程日志 (stdout 和 stderr 合并)
pmr logs <进程名>

# 查看最后 50 行
pmr logs <进程名> -n 50

# 查看轮转的日志文件
pmr logs <进程名> --rotated

# 手动轮转日志文件
pmr logs <进程名> --rotate
```

### 停止进程

```bash
pmr stop <进程名>
```

### 重启进程

```bash
pmr restart <进程名>
```

### 删除进程

```bash
pmr delete <进程名>
```

### 清空进程

```bash
# 清空已停止或失败的进程
pmr clear

# 清空所有进程（包括正在运行的进程）
pmr clear --all

# 使用 JSON 格式输出
pmr --format json clear
```

## HTTP API (可选功能)

PMR 支持可选的 HTTP API 功能，需要在编译时启用 `http-api` 特性。

### 启动 API 服务器

```bash
# 启动 HTTP API 服务器（前台运行，默认端口 8080）
pmr serve

# 指定端口
pmr serve --port 3000

# 启动 HTTP API 服务器（后台运行）
pmr serve --daemon

# 后台运行并指定端口
pmr serve --daemon --port 3000
```

### 管理 API 服务器

```bash
# 查看 HTTP API 服务器状态
pmr serve-status

# 停止 HTTP API 服务器
pmr serve-stop

# 重启 HTTP API 服务器（默认端口 8080）
pmr serve-restart

# 重启 HTTP API 服务器并指定端口
pmr serve-restart --port 3000
```

### 管理 API 认证令牌

```bash
# 生成新的 API 令牌
pmr auth generate my-token

# 生成有过期时间的令牌（30天后过期）
pmr auth generate my-token --expires-in 30

# 列出所有令牌
pmr auth list

# 撤销令牌
pmr auth revoke <token-string>
```

### API 文档

PMR 提供完整的 Swagger/OpenAPI 文档：

- **Swagger UI**: `http://localhost:8080/swagger-ui/` - 交互式 API 文档界面
- **OpenAPI JSON**: `http://localhost:8080/api-docs/openapi.json` - OpenAPI 规范文件

### API 端点

所有 API 请求都需要在 Header 中包含认证令牌：
```
Authorization: Bearer <your-token>
```

#### 进程管理端点

- `GET /api/processes` - 获取所有进程列表
- `POST /api/processes` - 启动新进程
- `GET /api/processes/{name}` - 获取指定进程状态
- `PUT /api/processes/{name}/stop` - 停止进程
- `PUT /api/processes/{name}/restart` - 重启进程
- `DELETE /api/processes/{name}` - 删除进程
- `GET /api/processes/{name}/logs` - 获取进程日志

#### API 使用示例

```bash
# 获取所有进程
curl -H "Authorization: Bearer <token>" http://localhost:8080/api/processes

# 启动新进程
curl -X POST -H "Authorization: Bearer <token>" \
     -H "Content-Type: application/json" \
     -d '{"name":"test","command":"sleep","args":["60"]}' \
     http://localhost:8080/api/processes

# 获取进程状态
curl -H "Authorization: Bearer <token>" http://localhost:8080/api/processes/test

# 停止进程
curl -X PUT -H "Authorization: Bearer <token>" \
     http://localhost:8080/api/processes/test/stop

# 获取进程日志
curl -H "Authorization: Bearer <token>" \
     http://localhost:8080/api/processes/test/logs
```

### 使用 Swagger UI

1. 启动 API 服务器：`pmr serve --port 8080`
2. 生成 API 令牌：`pmr auth generate my-token`
3. 在浏览器中打开：`http://localhost:8080/swagger-ui/`
4. 点击右上角的 "Authorize" 按钮
5. 输入 `Bearer <your-token>` 进行认证
6. 现在可以直接在 Swagger UI 中测试所有 API 端点

## 配置

PMR 使用以下目录结构：

- 数据库文件: `~/.pmr/processes.db`
- 默认日志目录: `./logs/` (相对于当前工作目录)
- 自定义日志目录: 可通过 `--log-dir` 参数指定

### 日志管理

- **日志文件**: 每个进程一个 `.log` 文件，包含 stdout 和 stderr
- **日志轮转**: 支持自动和手动日志轮转
  - 默认最大文件大小: 10MB
  - 默认保留轮转文件数: 5个
  - 轮转文件命名: `进程名.1.log`, `进程名.2.log`, 等
- **日志目录分离**: 日志文件和数据库文件存储在不同目录

这些目录会在首次运行时自动创建。

## 示例场景

### 1. 管理 Web 服务器

```bash
# 启动 Web 服务器
pmr start web-server python3 -m http.server -e PORT=8080 -w /var/www/html

# 查看状态
pmr status web-server

# 查看日志
pmr logs web-server

# 重启服务器
pmr restart web-server
```

### 2. 运行后台任务

```bash
# 启动数据备份任务
pmr start backup-job ./backup.sh -e BACKUP_DIR=/backup -w /home/user/scripts

# 查看任务状态
pmr list

# 查看备份日志
pmr logs backup-job
```

### 3. 开发环境管理

```bash
# 启动数据库
pmr start postgres postgres -D /var/lib/postgresql/data

# 启动 Redis
pmr start redis redis-server /etc/redis/redis.conf

# 启动应用服务器
pmr start -e NODE_ENV=development -w /path/to/app app-server npm start

# 查看所有服务状态
pmr list
```

### 4. 日志管理

```bash
# 启动一个会产生大量日志的服务，使用自定义日志目录
pmr start --log-dir /var/log/myservice -e LOG_LEVEL=debug my-service ./service.sh

# 查看当前日志
pmr logs my-service

# 查看最后100行日志
pmr logs my-service -n 100

# 手动轮转日志文件
pmr logs my-service --rotate

# 查看所有轮转的日志文件
pmr logs my-service --rotated

# 查看进程状态（包含日志文件路径）
pmr status my-service
```

### 5. 批量清理进程

```bash
# 启动多个测试进程
pmr start test1 echo "test1"
pmr start test2 echo "test2"
pmr start test3 sleep 60

# 查看所有进程状态
pmr list

# 清空已停止或失败的进程（保留正在运行的进程）
pmr clear

# 查看剩余进程
pmr list

# 清空所有进程（包括正在运行的进程）
pmr clear --all

# 使用 JSON 格式查看清理结果
pmr --format json clear
```


## 构建要求

- Rust 1.70+
- SQLite 3.x

## 依赖项

主要依赖：
- `clap` - 命令行解析
- `tokio` - 异步运行时
- `sqlx` - SQLite 数据库操作
- `serde` - 序列化/反序列化
- `chrono` - 时间处理
- `uuid` - 唯一ID生成

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

## 贡献

欢迎提交 Issue 和 Pull Request！
