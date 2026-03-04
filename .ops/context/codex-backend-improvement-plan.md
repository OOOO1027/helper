# CodeX 后端改进工作文档

> 面向 CodeX 后端 Thread · 基于 2026-03 架构审查  
> 状态: **待执行** | 优先级标记: 🔴 P0 🟡 P1 🟢 P2

---

## 一、架构总评

当前后端基于 Rust + SQLite(SQLCipher) + Tauri IPC 构建，核心数据流
`采集 → 管线 → 审核 → Notion 同步` 设计合理，已完成 150+ 测试通过。
但在可维护性、可测试性和可扩展性上存在以下系统性问题。

---

## 二、改进任务清单

### 🔴 P0 — 必须修复（影响正确性 / 生产稳定性）

#### B-P0-01: 消除数据库连接散布模式

**现状问题**  
`app_core` 及 `ipc/commands/mod.rs` 中大量方法直接调用 `open_conn_from_env()` 创建数据库连接。  
每次 IPC 调用都重新读取环境变量 → 打开加密连接 → 执行迁移检查，效率低、易出错。

```rust
// 当前模式（散布在 15+ 个方法中）
fn some_action() -> Result<T> {
    let conn = open_conn_from_env()?;  // 每次重复
    // ...业务逻辑
}
```

**改进方案**  
1. 在 `app_core/mod.rs` 中引入 `AppCore` 结构体，持有一个经过初始化的 `Connection` 或连接工厂：
   ```rust
   pub struct AppCore {
       db: DbPool,  // 可以是单连接包装或 r2d2 Pool
   }
   
   impl AppCore {
       pub fn new(config: DbConfig) -> Result<Self> { ... }
   }
   ```
2. 所有公开方法改为 `&self` 方法，通过 `self.db` 获取连接。
3. 在 Tauri `setup` 钩子中创建 `AppCore` 实例，注入到 Tauri `State`。

**涉及文件**  
- `backend/src/app_core/mod.rs` — 重构为带状态的 struct
- `backend/src/ipc/commands/mod.rs` — 移除直接 `open_conn_from_env` 调用
- `backend/src/ipc/commands/tauri_api.rs` — 从 Tauri State 获取 AppCore
- `backend/src/ipc/commands/publish.rs` — 同上

**验收标准**  
- [ ] 数据库连接只在启动时创建一次
- [ ] 所有 IPC 命令通过依赖注入获取连接
- [ ] 现有 150+ 测试全部通过

---

#### B-P0-02: 关键操作增加事务边界

**现状问题**  
多步操作（如 normalize → dedupe → persist → 更新状态）缺少事务包裹，  
中途失败会导致数据不一致（例如：内容已入库但状态未更新）。

**改进方案**  
1. 在 `storage/db.rs` 中封装事务辅助函数：
   ```rust
   pub fn with_transaction<F, T>(conn: &Connection, f: F) -> Result<T>
   where F: FnOnce(&Transaction) -> Result<T> { ... }
   ```
2. 对以下操作显式使用事务：
   - `import_wechat_files`（批量导入）
   - `run_pipeline`（管线执行）
   - `publish_approved_to_notion`（发布后状态更新）
   - `batch_review_update`（批量审核）
   - `replay_dead_letters`（死信重放）

**涉及文件**  
- `backend/src/storage/db.rs` — 增加 `with_transaction` 辅助
- `backend/src/app_core/wechat_import.rs`
- `backend/src/app_core/pipeline.rs`
- `backend/src/ipc/commands/publish.rs`
- `backend/src/app_core/dead_letter_recovery.rs`

**验收标准**  
- [ ] 所有多步写操作在事务内完成
- [ ] 事务失败时全部回滚，不留残留数据
- [ ] 增加一个集成测试验证中途失败的回滚行为

---

#### B-P0-03: 状态机 transition 返回 Result 而非静默保留

**现状问题**  
`pipeline/queue.rs::transition()` 在无效状态转换时返回原状态，调用方无法感知逻辑错误。

```rust
// 当前：无效转换被静默吞掉
(s, _) => s,
```

**改进方案**  
```rust
pub fn transition(state: PipelineState, event: PipelineEvent) -> Result<PipelineState> {
    match (state, event) {
        (PipelineState::Queued, PipelineEvent::Start) => Ok(PipelineState::Processing),
        // ...其他有效转换
        (s, e) => Err(BackendError::Validation(
            format!("invalid transition: {:?} + {:?}", s, e)
        )),
    }
}
```

**涉及文件**  
- `backend/src/pipeline/queue.rs` — 修改 transition 签名
- 所有调用 `transition()` 的位置 — 处理 Result
- 相关测试文件 — 增加无效转换测试

**验收标准**  
- [ ] `transition()` 返回 `Result<PipelineState>`
- [ ] 无效转换产生明确错误
- [ ] 增加测试覆盖至少 3 种无效转换路径

---

### 🟡 P1 — 应该改进（影响可维护性 / 开发效率）

#### B-P1-01: 统一重试逻辑，消除 DRY 违规

**现状问题**  
`pipeline/queue.rs::next_retry_delay_ms()` 和 `sync_notion/client.rs::RetryPolicy::backoff_ms()` 各自实现了指数退避，逻辑重复且参数不同。

**改进方案**  
1. 新建 `backend/src/retry.rs` 模块：
   ```rust
   pub struct RetryPolicy {
       pub max_attempts: u32,
       pub base_ms: u64,
       pub jitter_seed: u64,
   }
   
   impl RetryPolicy {
       pub fn delay_ms(&self, attempt: u32) -> Option<u64> { ... }
       pub fn should_give_up(&self, attempt: u32) -> bool { ... }
   }
   ```
2. `pipeline/queue.rs` 和 `sync_notion/client.rs` 改为使用共享 `RetryPolicy`。

**涉及文件**  
- 新建 `backend/src/retry.rs`
- `backend/src/lib.rs` — 增加 `pub mod retry;`
- `backend/src/pipeline/queue.rs` — 改用共享 RetryPolicy
- `backend/src/sync_notion/client.rs` — 改用共享 RetryPolicy

**验收标准**  
- [ ] 重试逻辑只有一处实现
- [ ] 两处使用场景的行为与改动前一致
- [ ] 增加单元测试验证边界条件（attempt=0, max, overflow）

---

#### B-P1-02: 拆分 AppCore 为领域服务

**现状问题**  
`app_core/mod.rs` 包含 70+ 公开方法，覆盖仪表盘、审核、导入、采集、管线、同步分析等全部业务逻辑。  
违反单一职责原则，新增功能时认知负担高。

**改进方案**  
保持 `AppCore` 作为门面（facade），将实现拆分到已有的子模块中，各子模块暴露独立的 Service trait：

| 子模块 | 职责 | 对应文件 |
|--------|------|----------|
| `dashboard` | 仪表盘指标 | `app_core/dashboard.rs` |
| `review_queue` | 审核队列 CRUD | `app_core/review_queue.rs` |
| `wechat_import` | 微信导入 | `app_core/wechat_import.rs` |
| `xhs_collection` | 小红书采集 | `app_core/xhs_collection.rs` |
| `pipeline` | 管线执行 | `app_core/pipeline.rs` |
| `sync_analytics` | 同步统计 | `app_core/sync_analytics.rs` |
| `dead_letter_recovery` | 死信恢复 | `app_core/dead_letter_recovery.rs` |

每个子模块定义自己的 public 函数签名，`AppCore` 只做委托：
```rust
impl AppCore {
    pub fn dashboard_metrics(&self, ...) -> Result<DashboardMetrics> {
        dashboard::get_metrics(&self.db, ...)
    }
}
```

**验收标准**  
- [ ] `app_core/mod.rs` 仅包含 struct 定义、构造方法和委托调用
- [ ] 各子模块可独立编译和测试
- [ ] 公开 API 签名不变，不影响 IPC 层

---

#### B-P1-03: 增强错误上下文链

**现状问题**  
`BackendError::Internal(String)` 过于通用，许多 `.map_err(|e| BackendError::Internal(e.to_string()))` 丢失了错误链。

**改进方案**  
1. 为常用错误源增加专用变体：
   ```rust
   #[derive(Debug, Error)]
   pub enum BackendError {
       #[error("validation: {0}")]
       Validation(String),
       #[error("not found: {0}")]
       NotFound(String),
       #[error("storage: {0}")]
       Storage(#[from] rusqlite::Error),
       #[error("network: {0}")]
       Network(#[from] reqwest::Error),    // 新增
       #[error("io: {0}")]
       Io(#[from] std::io::Error),         // 新增
       #[error("config: {context}")]
       Config { context: String },          // 新增
       #[error("internal: {0}")]
       Internal(String),
   }
   ```
2. 更新 `ErrorCode` 映射。
3. 逐步替换 `BackendError::Internal(e.to_string())` 为具体变体。

**涉及文件**  
- `backend/src/lib.rs` — 扩展 BackendError 枚举
- 全局搜索 `BackendError::Internal` 的调用点，逐一替换

**验收标准**  
- [ ] `reqwest::Error` 和 `std::io::Error` 有专用变体
- [ ] 配置错误有专用变体并包含字段名
- [ ] 编译通过，现有测试通过

---

#### B-P1-04: 硬编码阈值常量化

**现状问题**  
质量门控阈值（65、78）、预算百分比（85%、95%）、重试次数（3）等魔法数字散布在多个文件中。

**改进方案**  
1. 在 `backend/src/config/mod.rs` 中集中定义常量或可配置默认值：
   ```rust
   // 质量门控
   pub const GATE_DIRECT_THRESHOLD: f64 = 78.0;
   pub const GATE_REVIEW_THRESHOLD: f64 = 65.0;
   
   // 预算
   pub const BUDGET_REVIEW_OFF_PCT: f64 = 0.85;
   pub const BUDGET_DAILY_OFF_PCT: f64 = 0.95;
   
   // 重试
   pub const MAX_RETRY_ATTEMPTS: u32 = 3;
   ```
2. 各模块引用这些常量而非硬编码。

**涉及文件**  
- `backend/src/config/mod.rs` — 新增常量
- `backend/src/pipeline/score.rs` — 引用门控阈值
- `backend/src/budget_guard/mod.rs` — 引用预算阈值
- `backend/src/pipeline/queue.rs` — 引用重试上限

**验收标准**  
- [ ] 所有阈值在 config 模块集中定义
- [ ] 业务代码中无直接硬编码数字
- [ ] 修改阈值只需改一处

---

#### B-P1-05: 增加外部服务抽象层

**现状问题**  
`sync_notion/service/` 直接构建 `NotionHttpClient`，测试中难以 mock。  
AI 接口同样缺乏 trait 抽象。

**改进方案**  
1. 为 Notion API 定义 trait：
   ```rust
   #[async_trait]
   pub trait NotionApi: Send + Sync {
       async fn create_page(&self, ...) -> Result<String>;
       async fn update_page(&self, ...) -> Result<()>;
       async fn query_database(&self, ...) -> Result<Vec<Value>>;
   }
   ```
2. `NotionHttpClient` 实现该 trait，测试用 `MockNotionApi` 替代。
3. 类似地为 AI 模型调用定义 `AiModelApi` trait。

**涉及文件**  
- `backend/src/sync_notion/client.rs` — 提取 trait
- `backend/src/sync_notion/service/mod.rs` — 改用 trait 对象
- `backend/src/ai_orchestrator/router.rs` — 提取 AiModelApi trait

**验收标准**  
- [ ] Notion 和 AI 有独立的 trait 定义
- [ ] Service 层通过 trait 对象/泛型接收依赖
- [ ] 可以用 mock 实现编写纯单元测试

---

### 🟢 P2 — 建议改进（提升代码质量和开发体验）

#### B-P2-01: 增加纯函数单元测试

**现状问题**  
当前 26 个测试全部是集成测试，依赖环境变量和数据库。  
评分、去重、归一化等纯函数缺少独立的单元测试。

**改进方案**  
在各模块内增加 `#[cfg(test)]` 模块：
- `pipeline/score.rs` — 边界值测试（0/100、负数、NaN 输入）
- `pipeline/dedupe.rs` — 去重规则测试（相同 URL、相似内容、时间窗口）
- `pipeline/normalize.rs` — URL 规范化、文本清洗
- `budget_guard/mod.rs` — 状态转换测试

**验收标准**  
- [ ] 每个纯函数模块至少 5 个单元测试
- [ ] 覆盖正常路径、边界值和错误路径
- [ ] `cargo test` 无需环境变量即可运行单元测试

---

#### B-P2-02: 为 SQLite 适配层消除 unwrap_or 静默失败

**现状问题**  
`adapters/sqlite_core_port.rs` 中大量 `.unwrap_or(0.0)` 在取值失败时静默返回零值，可能掩盖数据损坏。

**改进方案**  
1. 改为 `map_err` + `tracing::warn!` 记录异常后返回默认值：
   ```rust
   row.get::<_, f64>(0).unwrap_or_else(|e| {
       tracing::warn!("column read failed: {e}, defaulting to 0.0");
       0.0
   })
   ```
2. 对于关键字段（如 `classification_accuracy`），考虑向上传播错误而非默认值。

**涉及文件**  
- `backend/src/app_core/adapters/sqlite_core_port.rs`

**验收标准**  
- [ ] 所有 `unwrap_or` 改为 `unwrap_or_else` 并带日志
- [ ] 关键数据字段失败时返回错误而非默认值

---

#### B-P2-03: 引入 Clippy 更严格的配置

**现状问题**  
`clippy.toml` 中 `cognitive_complexity_threshold = 30` 过于宽松，应促使复杂函数拆分。

**改进方案**  
1. 将阈值调整为 `cognitive_complexity_threshold = 15`
2. 修复现有超限函数（拆分或提取子函数）
3. 在 `Cargo.toml` 或 crate 级别启用额外 clippy lint 组：
   ```toml
   # clippy.toml
   cognitive-complexity-threshold = 15
   ```

**验收标准**  
- [ ] `cargo clippy -- -D warnings` 在新阈值下通过
- [ ] 复杂函数已拆分到阈值以下

---

## 三、执行顺序建议

```
Phase 1（正确性）: B-P0-01 → B-P0-02 → B-P0-03
Phase 2（可维护）: B-P1-01 → B-P1-04 → B-P1-03 → B-P1-02 → B-P1-05
Phase 3（质量）  : B-P2-01 → B-P2-02 → B-P2-03
```

每完成一个任务后执行：
```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

---

## 四、风险与注意事项

| 风险 | 缓解措施 |
|------|----------|
| B-P0-01 改动范围大 | 先改 AppCore struct，再逐步迁移各 IPC 命令 |
| B-P0-02 事务边界引入死锁 | SQLite WAL 模式天然减少冲突，保持短事务 |
| B-P1-02 拆分可能破坏 Tauri feature gate | 保持 `#[cfg(feature)]` 注解位置不变 |
| B-P1-05 mock 测试需要额外 dev-dependency | 仅添加 `mockall` 到 `[dev-dependencies]` |
