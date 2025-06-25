# PMR - Process Manager in Rust

PMR 是一个用 Rust 编写的 Linux 进程管理工具，提供了完整的进程生命周期管理功能。

## 特性

- **系统级进程管理** - 管理的进程独立于工具运行，工具退出不会影响被管理的进程
- **隔离的环境变量** - 每个进程都有独立的环境变量配置
- **完整的进程操作** - 支持启动、停止、重启、删除进程
- **日志管理** - 自动捕获和管理进程的 stdout 和 stderr
- **持久化存储** - 进程配置和状态持久化存储
- **命令行界面** - 简洁易用的命令行接口

## 安装

```bash
# 克隆项目
git clone <repository-url>
cd pmr

# 构建项目
cargo build --release

# 安装到系统路径（可选）
cargo install --path .
```

## 使用方法

### 基本命令

#### 启动进程
```bash
# 启动一个简单的进程
pmr start my-app /usr/bin/my-application

# 启动带参数的进程
pmr start web-server nginx -g "daemon off;"

# 启动时设置工作目录
pmr start my-service --workdir /opt/myapp ./start.sh

# 启动时设置环境变量
pmr start api-server --env PORT=8080 --env ENV=production ./server
```

#### 查看进程
```bash
# 列出所有进程
pmr list

# 查看进程详细信息
pmr describe my-app
```

#### 控制进程
```bash
# 停止进程
pmr stop my-app

# 重启进程
pmr restart my-app

# 删除进程配置（需要先停止进程）
pmr delete my-app
```

#### 查看日志
```bash
# 查看进程日志（默认显示最后50行）
pmr logs my-app

# 查看指定行数的日志
pmr logs my-app --lines 100

# 实时跟踪日志
pmr logs my-app --follow
```

#### 环境变量管理
```bash
# 为进程设置环境变量（需要先停止进程）
pmr env my-app DATABASE_URL=postgresql://localhost/mydb
pmr env my-app API_KEY=secret123 DEBUG=true
```

### 高级用法

#### 启动时的完整配置
```bash
pmr start web-app \
  --workdir /var/www/html \
  --env PORT=3000 \
  --env NODE_ENV=production \
  node server.js
```

#### 管理多个相关进程
```bash
# 启动数据库
pmr start database postgres -D /var/lib/postgresql/data

# 启动 Redis
pmr start cache redis-server /etc/redis/redis.conf

# 启动 Web 应用
pmr start webapp --env DB_HOST=localhost --env REDIS_HOST=localhost ./app

# 查看所有进程状态
pmr list
```

## 数据存储

PMR 将进程配置和状态存储在用户数据目录中：
- Linux: `~/.local/share/pmr/`

存储的文件包括：
- `processes.json` - 进程配置和状态
- `<process-name>.stdout.log` - 进程标准输出日志
- `<process-name>.stderr.log` - 进程标准错误日志

## 进程状态

PMR 跟踪以下进程状态：
- **running** - 进程正在运行
- **stopped** - 进程已停止
- **failed** - 进程异常退出
- **unknown** - 进程状态未知

## 注意事项

1. **权限要求** - 确保有足够的权限启动目标进程
2. **进程独立性** - 被管理的进程会在后台独立运行，不受 PMR 工具退出影响
3. **环境变量修改** - 只能在进程停止状态下修改环境变量
4. **日志轮转** - 当前版本不支持自动日志轮转，需要手动管理日志文件大小

## 示例场景

### Web 服务管理
```bash
# 启动 Nginx
pmr start nginx nginx -g "daemon off;"

# 启动 Node.js 应用
pmr start api --env PORT=3000 --workdir /opt/myapp node app.js

# 查看服务状态
pmr list

# 查看应用日志
pmr logs api --follow
```

### 开发环境管理
```bash
# 启动开发数据库
pmr start dev-db --env POSTGRES_DB=myapp_dev postgres

# 启动开发服务器
pmr start dev-server --env NODE_ENV=development npm start

# 重启服务器
pmr restart dev-server
```

## 故障排除

### 进程启动失败
1. 检查命令路径是否正确
2. 确认有执行权限
3. 查看错误日志：`pmr logs <process-name>`

### 进程状态不同步
进程状态可能不会立即更新。如果发现状态不准确，可以：
1. 重新查看进程列表：`pmr list`
2. 检查系统进程：`ps aux | grep <process-name>`

### 日志文件过大
定期清理日志文件：
```bash
# 查看日志文件位置
pmr describe <process-name>

# 手动清理日志
> ~/.local/share/pmr/<process-name>.stdout.log
> ~/.local/share/pmr/<process-name>.stderr.log
```

## 贡献

欢迎提交 Issue 和 Pull Request 来改进这个项目。

## 许可证

MIT License
