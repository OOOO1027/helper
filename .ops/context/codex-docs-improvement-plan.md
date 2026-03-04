# CodeX 文档改进工作文档

> 面向 CodeX 文档 Thread · 基于 2026-03 架构审查  
> 状态: **待执行** | 优先级标记: 🔴 P0 🟡 P1 🟢 P2

---

## 一、文档现状总评

项目已有较完善的架构文档体系（`.ops/context/` 下 15 个文档 + 4 个状态文件），
包括 master-spec、ADR、决策日志、验收模板等。README 基本可用。
但存在文档碎片化、缺少开发者日常文档、API 文档缺失等问题。

---

## 二、改进任务清单

### 🔴 P0 — 必须修复（影响新人上手 / 日常开发）

#### D-P0-01: 补充 CONTRIBUTING.md 开发者指南

**现状问题**  
新开发者（包括 CodeX Agent）没有统一的贡献指南。  
开发流程、分支策略、提交规范、代码审查要求散落在不同文档中或完全缺失。

**改进方案**  
在项目根目录创建 `CONTRIBUTING.md`，包含以下章节：

```markdown
# 贡献指南

## 开发环境搭建
- 前置依赖（Rust、Node.js、pnpm 版本要求）
- 环境变量配置（指向 .env.example）
- 首次运行步骤

## 项目结构概览
- backend/、desktop/、.ops/ 各目录职责
- 关键文件说明（lib.rs、App.tsx、master-spec.md）

## 开发工作流
- 分支命名规范（feature/、fix/、docs/）
- 提交信息格式（conventional commits）
- PR 审查流程
- CI 检查项说明

## 代码规范
- Rust: rustfmt + clippy 配置说明
- TypeScript: ESLint + Prettier 配置说明
- 命名规范（Rust snake_case、TS camelCase）

## 测试要求
- 后端：cargo test 运行方式、环境变量要求
- 前端：pnpm build + pnpm lint 检查
- 新功能必须附带测试

## 文档规范
- 架构决策记录 ADR 流程
- decision-log.md 更新规则
- 状态文件更新时机
```

**涉及文件**  
- 新建 `CONTRIBUTING.md`

**验收标准**  
- [ ] 新开发者按文档可完成从 clone 到运行的全流程
- [ ] 包含 Rust 和 TypeScript 两套工具链的说明
- [ ] 包含提交和 PR 规范

---

#### D-P0-02: 创建 .env.example 环境变量模板

**现状问题**  
README 中列出了环境变量，但没有可直接复制使用的模板文件。  
开发者需要手动逐个创建，容易遗漏或格式错误。

**改进方案**  
在项目根目录创建 `.env.example`：

```bash
# ── 数据库（必须）──────────────────────────────────
HELPER_DB_PATH=./data/helper.db
HELPER_DB_KEY=change_me_to_random_key

# ── Notion 同步（至少配一组）─────────────────────
NOTION_TOKEN=secret_xxx
NOTION_SYNC_MODE=page_tree
# page_tree 模式
NOTION_ROOT_PAGE_ID=
# database 模式（备选）
NOTION_DATABASE_ID=

# ── AI 分析（可选）────────────────────────────────
QWEN_API_KEY=sk-xxx

# ── 预算控制 ──────────────────────────────────────
# BUDGET_MONTHLY_LIMIT_CNY=100

# ── 开发调试 ──────────────────────────────────────
# RUST_LOG=helper_backend=debug
# HELPER_DRY_RUN=true
```

**涉及文件**  
- 新建 `.env.example`
- `.gitignore` — 确认已有 `!.env.example` 排除规则（已确认存在）

**验收标准**  
- [ ] `.env.example` 包含所有已知环境变量
- [ ] 每个变量有中文注释说明用途
- [ ] `.gitignore` 不会忽略此文件

---

#### D-P0-03: 补充 IPC API 参考文档

**现状问题**  
前后端的 IPC 合约分散在 `backend/contracts/ipc-sync-examples.json`、
`desktop/src/types/contracts.ts`、`.ops/context/publish-ipc-contract-v1.md` 三处。  
缺少一个完整的、面向开发者的 API 参考文档。

**改进方案**  
在 `.ops/context/` 创建 `ipc-api-reference.md`，包含：

```markdown
# IPC API 参考

## 概述
前端通过 Tauri invoke 调用后端命令，所有命令定义在
backend/src/ipc/commands/ 下。

## 命令清单

### 数据采集
| 命令 | 请求类型 | 响应类型 | 说明 |
|------|----------|----------|------|
| `collect` | CollectReq | CollectResult | 触发内容采集 |
| `import_wechat_files` | ImportWechatFilesReq | ImportPersistSummary | 导入微信文件 |

### 审核队列
| 命令 | 请求类型 | 响应类型 | 说明 |
|------|----------|----------|------|
| `get_review_items` | GetReviewItemsReq | Paged<ReviewItem> | 分页获取审核项 |
| `update_review_item` | UpdateReviewItemReq | bool | 更新审核状态 |
| `batch_review` | BatchReviewReq | BatchReviewResult | 批量审核 |

### 发布
| 命令 | 请求类型 | 响应类型 | 说明 |
|------|----------|----------|------|
| `get_publish_queue` | PageReq | Paged<PublishTask> | 获取发布队列 |
| `publish_approved_to_notion` | — | SyncRunSummary | 发布到 Notion |
| `get_publish_history` | filters | Paged<PublishAudit> | 发布历史 |

### 系统管理
（dashboard、health、dead_letter 等命令）

## 错误码
| 码 | 含义 | 可重试 |
|----|------|--------|
| IPC-6001 | 参数校验失败 | ❌ |
| ING-1001 | 不支持的采集源 | ❌ |
| DB-4001 | 数据库操作失败 | ✅ |
| INT-9000 | 内部错误 | ✅ |

## 类型定义文件位置
- Rust: backend/src/ipc/commands/mod.rs
- TypeScript: desktop/src/types/contracts.ts
- JSON 示例: backend/contracts/ipc-sync-examples.json
```

**涉及文件**  
- 新建 `.ops/context/ipc-api-reference.md`

**验收标准**  
- [ ] 覆盖所有已实现的 IPC 命令
- [ ] 每个命令包含请求/响应类型和说明
- [ ] 包含错误码参考
- [ ] 指明类型定义的源文件位置

---

### 🟡 P1 — 应该改进（影响文档可维护性和一致性）

#### D-P1-01: 统一 master-spec.md 与实际代码的一致性

**现状问题**  
`master-spec.md` 标注为 DRAFT 状态，部分内容（如页面导航结构、数据模型字段）可能与
最新代码不一致。作为"单一事实来源"（SSOT），必须保持准确。

**改进方案**  
1. 逐节审查 `master-spec.md`，对照代码更新：
   - 导航结构（4 页面 vs 7 页面文件的差异说明）
   - IPC 命令列表（确认完整性）
   - 数据模型（对照最新 migration SQL）
   - 配置项（对照 `config/mod.rs`）
2. 标注未实现和已废弃的部分。
3. 将状态从 DRAFT 更新为 v1.0-beta。

**涉及文件**  
- `.ops/context/master-spec.md`

**验收标准**  
- [ ] master-spec 中的所有 IPC 命令与代码一致
- [ ] 数据模型与最新 migration 一致
- [ ] 已废弃的功能明确标注
- [ ] 状态更新为 v1.0-beta

---

#### D-P1-02: 完善 README 快速上手指南

**现状问题**  
README 的"快速开始"缺少端到端运行验证步骤。  
新开发者不知道如何验证环境搭建成功。

**改进方案**  
在 README 的"快速开始"部分增加：

```markdown
### 验证环境

# 1. 后端编译和测试
cd backend
cargo build
cargo test  # 应看到 "test result: ok. XX passed"

# 2. 前端类型检查和构建
cd desktop
pnpm install
pnpm build  # 应看到 "Build successful"

# 3. 启动桌面应用
pnpm tauri dev  # 应看到应用窗口打开

### 常见问题

| 问题 | 解决方案 |
|------|----------|
| `sqlcipher` 链接失败 | 确保已安装 OpenSSL 开发库 |
| Tauri 构建失败 | 检查系统依赖: `xcode-select --install` (macOS) |
| 环境变量未生效 | 检查 `.env` 文件位置和格式 |
```

**涉及文件**  
- `README.md`

**验收标准**  
- [ ] 验证步骤与实际行为一致
- [ ] 常见问题覆盖前 3 名高频问题
- [ ] 步骤编号清晰，可逐步执行

---

#### D-P1-03: 为 ADR 建立索引和模板

**现状问题**  
4 个 ADR 文档缺少编号索引，新增 ADR 时没有统一模板。

**改进方案**  
1. 创建 `ADR-index.md` 索引：
   ```markdown
   # 架构决策记录索引

   | 编号 | 标题 | 状态 | 日期 |
   |------|------|------|------|
   | ADR-001 | AI 预算守卫 | ✅ Accepted | 2026-02 |
   | ADR-002 | 信息架构 v2 | ✅ Accepted | 2026-02 |
   | ADR-003 | Notion 作为最终展示面 | ✅ Accepted | 2026-02 |
   | ADR-004 | 发布领域模型 | ✅ Accepted | 2026-02 |
   ```

2. 创建 `ADR-template.md` 模板：
   ```markdown
   # ADR-NNN: [标题]

   ## 状态
   提议 / 已接受 / 已废弃 / 已取代

   ## 上下文
   [描述促使此决策的背景和问题]

   ## 决策
   [描述做出的决策]

   ## 后果
   [描述决策的正面和负面影响]

   ## 参考
   [相关文档、讨论链接]
   ```

**涉及文件**  
- 新建 `.ops/context/ADR-index.md`
- 新建 `.ops/context/ADR-template.md`

**验收标准**  
- [ ] 索引覆盖所有现有 ADR
- [ ] 模板包含状态、上下文、决策、后果四个必填节
- [ ] 新建 ADR 时有明确的流程指引

---

#### D-P1-04: 补充数据库 Schema 文档

**现状问题**  
数据库有 11 个 migration 文件，但缺少一个完整的 Schema 概览文档。  
开发者需要逐个阅读 migration SQL 才能理解表结构。

**改进方案**  
创建 `.ops/context/database-schema.md`，包含：

```markdown
# 数据库 Schema 概览

## 表清单
| 表名 | 用途 | 关键字段 |
|------|------|----------|
| normalized_items | 归一化后的内容 | id, source, title, ... |
| review_queue | 审核队列 | item_id, state, priority, ... |
| publish_tasks | 发布任务 | id, item_id, status, ... |
| notion_page_refs | Notion 页面映射 | item_id, page_id, ... |
| notion_tree_nodes | 页面树节点 | node_id, parent_id, ... |
| publish_audit | 发布审计日志 | request_id, status, ... |
| dead_letter | 死信队列 | id, original_table, ... |
| ai_budget_ledger | AI 预算账本 | month, used, limit, ... |
| schema_migrations | 迁移记录 | version, applied_at |

## ER 关系描述
（描述表间外键关系）

## 迁移历史
| 版本 | 文件 | 说明 |
|------|------|------|
| 001 | init | 初始表结构 |
| ... | ... | ... |
| 011 | notion_route_reason_tracking | Notion 路由原因追踪 |
```

**涉及文件**  
- 新建 `.ops/context/database-schema.md`

**验收标准**  
- [ ] 覆盖所有数据库表
- [ ] 包含关键字段说明和表间关系
- [ ] 与最新 migration 一致

---

### 🟢 P2 — 建议改进（提升文档体验）

#### D-P2-01: 增加架构图（Mermaid）

**现状问题**  
README 使用纯文本描述数据流，缺少可视化的架构图。

**改进方案**  
在 README 或单独文档中使用 Mermaid 语法绘制：

1. **系统架构图**：
   ```mermaid
   graph TD
     A[微信/小红书/B站] --> B[Collectors]
     B --> C[Pipeline]
     C --> D[Review Queue]
     D --> E[Notion Sync]
     F[AI Orchestrator] --> C
     G[Budget Guard] --> F
     H[SQLite/SQLCipher] --- B & C & D & E
   ```

2. **数据流图**（normalize → dedupe → analyze → score → review → publish）

3. **IPC 通信图**（Tauri 前端 ↔ Rust 后端）

**涉及文件**  
- `README.md` — 增加 Mermaid 图
- 可选：`.ops/context/architecture-diagrams.md`

**验收标准**  
- [ ] GitHub 渲染时能正确显示 Mermaid 图
- [ ] 至少包含系统架构和数据流两张图

---

#### D-P2-02: 统一状态文件格式

**现状问题**  
`.ops/status/` 下的 4 个文件格式不统一：
- `backend-latest.md` 和 `frontend-latest.md` 使用不同的结构
- `director-decisions.md` 和 `director-open-issues.md` 的更新频率不一致

**改进方案**  
1. 为状态文件定义统一模板：
   ```markdown
   # [模块] 状态

   > 更新时间: YYYY-MM-DD HH:MM CST  
   > 总体状态: GO / GO(Guarded) / BLOCKED

   ## 本轮完成
   - [x] 任务 1
   - [x] 任务 2

   ## 进行中
   - [ ] 任务 3（进度 XX%）

   ## 阻塞项
   - 阻塞描述及解决方案

   ## 下一步
   - 计划任务

   ## 指标
   | 指标 | 值 |
   |------|------|
   | 测试通过 | XX/XX |
   | 构建状态 | ✅/❌ |
   ```

2. 按模板重新格式化现有状态文件。

**涉及文件**  
- `.ops/status/backend-latest.md`
- `.ops/status/frontend-latest.md`
- `.ops/status/director-decisions.md`
- `.ops/status/director-open-issues.md`

**验收标准**  
- [ ] 4 个状态文件使用统一模板
- [ ] 包含更新时间、总体状态、任务清单、指标

---

#### D-P2-03: 补充安全文档

**现状问题**  
项目使用 SQLCipher 加密、处理 Notion API Token 等敏感数据，但缺少安全实践文档。

**改进方案**  
创建 `.ops/context/security-practices.md`：

```markdown
# 安全实践

## 数据加密
- 数据库使用 SQLCipher 加密，密钥通过 HELPER_DB_KEY 环境变量提供
- 密钥不得硬编码或提交到版本控制

## API 密钥管理
- NOTION_TOKEN 通过环境变量注入，不从 shell fallback 获取
- QWEN_API_KEY 仅在运行时读取

## 日志安全
- 日志中不得包含 API 密钥、用户数据明文
- 使用 tracing 框架的结构化日志

## CSP 策略
- Tauri 已配置 Content-Security-Policy
- 限制 script-src，允许 unsafe-inline styles

## 依赖安全
- 使用 cargo-audit 和 npm audit 定期检查
- 新增依赖前需通过 advisory database 检查
```

**涉及文件**  
- 新建 `.ops/context/security-practices.md`

**验收标准**  
- [ ] 覆盖加密、密钥管理、日志安全、CSP、依赖审计
- [ ] 与实际代码中的安全措施一致

---

## 三、执行顺序建议

```
Phase 1（必备）  : D-P0-01 → D-P0-02 → D-P0-03
Phase 2（一致性）: D-P1-01 → D-P1-02 → D-P1-03 → D-P1-04
Phase 3（体验）  : D-P2-01 → D-P2-02 → D-P2-03
```

---

## 四、文档维护规则

| 规则 | 说明 |
|------|------|
| **代码变更同步** | 修改 IPC 命令时同步更新 `ipc-api-reference.md` |
| **ADR 流程** | 新架构决策必须先创建 ADR，经审查后标记 Accepted |
| **状态更新** | 每个 Sprint 结束时更新 `.ops/status/` 文件 |
| **Schema 同步** | 新增 migration 时更新 `database-schema.md` |
| **安全审查** | 新增外部依赖时更新 `security-practices.md` |

---

## 五、与后端/前端文档的交叉引用

| 改进项 | 关联后端任务 | 关联前端任务 |
|--------|-------------|-------------|
| D-P0-03 IPC API 参考 | B-P0-01 AppCore 重构 | F-P1-03 IPC 缓存 |
| D-P1-01 master-spec 同步 | B-P1-02 领域拆分 | F-P1-01 状态管理 |
| D-P1-04 Schema 文档 | B-P0-02 事务边界 | — |
| D-P2-03 安全文档 | B-P1-03 错误上下文 | F-P0-01 ErrorBoundary |
