# PMR - Process Management Tool

PMR (Process Manager in Rust) 是一个用 Rust 编写的命令行进程管理工具，支持系统级进程管理、独立环境变量、日志管理和并发操作。

## 特性

- **系统级进程管理**: 进程独立于工具运行，不会因为工具退出而终止
- **隔离的环境变量**: 每个进程都有独立的环境变量配置
- **完整的进程操作**: 支持启动、停止、重启、删除进程
- **日志管理**: 自动捕获和管理进程的 stdout 和 stderr
- **SQLite 存储**: 使用 SQLite 数据库存储进程信息，支持并发操作
- **状态监控**: 实时监控进程状态

## 安装

```bash
# 克隆仓库
git clone <repository-url>
cd pmr

# 编译
cargo build --release

# 可选：安装到系统路径
cargo install --path .
```

## 使用方法

### 启动进程

```bash
# 基本用法
pmr start <进程名> <命令> [参数...]

# 示例：启动一个简单的进程
pmr start my-sleep sleep 60

# 使用环境变量
pmr start web-server python3 server.py -e PORT=8080 -e DEBUG=true

# 指定工作目录
pmr start my-app ./app.sh -w /path/to/workdir

# 复杂示例
pmr start nginx nginx -e NGINX_PORT=80 -w /etc/nginx

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

## 配置

PMR 使用以下目录结构：

- 数据库文件: `~/.pmr/processes.db`
- 日志目录: `~/.pmr/logs/` (每个进程一个 `.log` 文件，包含 stdout 和 stderr)

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
pmr start app-server npm start -e NODE_ENV=development -w /path/to/app

# 查看所有服务状态
pmr list
```

## 技术细节

- **进程分离**: 使用 `setsid` 创建新的会话，确保进程独立运行
- **数据库**: SQLite 数据库存储进程元数据，支持并发访问
- **日志管理**: 自动重定向 stdout/stderr 到统一的日志文件
- **状态监控**: 使用系统调用检查进程是否仍在运行
- **参数解析**: 支持传递带有 `-` 和 `--` 的命令参数，自动区分 pmr 选项和目标命令参数

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

Copyright (c) 2025 ArlenWen

## 贡献

欢迎提交 Issue 和 Pull Request！
