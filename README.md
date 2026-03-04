# Helper

AI 驱动的内容聚合与 Notion 同步桌面工具。

从微信、小红书、B站采集内容 → AI 分析 → 人工审核 → 同步到 Notion。

## 技术栈

| 层 | 技术 |
|----|------|
| 桌面框架 | [Tauri 2](https://tauri.app) |
| 后端 | Rust 2021 edition |
| 前端 | TypeScript + React 18 + Vite 5 |
| 数据库 | SQLite（SQLCipher 加密）|
| AI | Qwen API（阿里通义千问）|
| 同步目标 | Notion（database 模式 / page_tree 模式）|

## 项目结构

```
helper/
├── backend/               # Rust 后端（Tauri 集成）
│   ├── src/
│   │   ├── app_core/      # 核心业务逻辑
│   │   ├── ai_orchestrator/
│   │   ├── budget_guard/  # AI 用量预算控制
│   │   ├── collectors/    # 内容采集器（xhs / wechat / bili）
│   │   ├── ipc/           # IPC 命令层
│   │   │   └── commands/
│   │   │       ├── mod.rs       # 共享类型 + 业务函数
│   │   │       ├── publish.rs   # 发布管道
│   │   │       └── tauri_api.rs # Tauri 命令封装
│   │   ├── pipeline/      # 内容处理管道（normalize→analyze→dedupe→score）
│   │   ├── storage/       # 数据库层 + migrations
│   │   └── sync_notion/   # Notion 同步
│   │       └── service/
│   │           ├── mod.rs    # 同步编排
│   │           └── render.rs # Notion block 渲染
│   └── tests/             # 集成测试
├── desktop/               # TypeScript/React 前端
│   └── src/
│       ├── pages/         # 7 个主页面
│       ├── components/    # 可复用组件
│       └── services/      # API 适配层
├── data/                  # 运行时数据（.gitignore'd）
└── .ops/                  # 架构文档与决策记录
    ├── context/           # master-spec, ADR, decision-log
    └── status/            # 前后端进度状态
```

## 快速开始

### 环境要求

- Rust stable（`rustup update stable`）
- Node.js 20+，pnpm 9+
- macOS（依赖 AppleScript / textutil）

### 环境变量

复制并填写环境变量：

```bash
# 必须
HELPER_DB_PATH=/path/to/data/helper.db
HELPER_DB_KEY=your_db_encryption_key

# Notion 同步（至少配置其中一组）
NOTION_TOKEN=secret_xxx
NOTION_SYNC_MODE=page_tree          # 或 database
NOTION_ROOT_PAGE_ID=xxx             # page_tree 模式
NOTION_DATABASE_ID=xxx              # database 模式

# AI 分析（可选）
QWEN_API_KEY=sk-xxx
```

### 开发运行

```bash
# 后端（库 + 测试）
cd backend
cargo test

# 桌面应用
cd desktop
pnpm install
pnpm tauri dev
```

### 代码质量

```bash
# Rust
cd backend
cargo fmt
cargo clippy -- -D warnings
cargo test

# TypeScript
cd desktop
pnpm lint
pnpm format:check
pnpm build
```

## 架构文档

详见 `.ops/context/`：

- `master-spec.md` — 单一事实来源，API 契约，数据模型
- `decision-log.md` — 架构决策记录
- `ADR-*.md` — 各专项架构决策

## 数据流

```
采集源（XHS / 微信 / B站）
    ↓
Collectors（采集）
    ↓
Pipeline（normalize → analyze → dedupe → score）
    ↓
Review Queue（人工审核）
    ↓
Notion Sync（database 或 page_tree 模式）
```

## CI

Push 到 `main`/`master` 时触发 `.github/workflows/ci.yml`：
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `tsc --noEmit`
- `eslint`
- `prettier --check`
