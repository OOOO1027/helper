# Release Gates v1

Status: ACTIVE  
Owner: Director + FE Lead + BE Lead  
Last updated: 2026-02-26

## Purpose

把“文档先行、可执行验收、可回滚”固化为两道门：
- Gate A：契约与 ADR 门禁（先文档后实现）。
- Gate B：联调与闭环门禁（先证据后放行）。

## Gate A（契约/ADR 门）

### 入场条件（全部满足）
- [ ] 已完成 mandatory pre-read：`master-spec`、能力映射、IPC 合同、相关 ADR、decision-log。
- [ ] 本轮变更范围已声明：涉及 IPC、边界、预算或门禁策略之一。
- [ ] 责任人与决策级别已确认（A/B/C）。

### 出场条件（全部满足）
- [ ] IPC 合同已更新到可执行级（请求字段、筛选语义、错误样例、兼容语义完整）。
- [ ] 所有重大边界决策均有 ADR（含 Notion 最终呈现边界与 App 处理引擎边界）。
- [ ] 预算策略有独立 ADR，明确阈值、降级顺序、回滚条件。
- [ ] `decision-log.md` 已追加本轮决策（日期、决策人、影响范围、回滚条件）。
- [ ] 文档规则与现行实现无冲突（至少完成一次代码核验：命令签名/迁移/错误码）。

### Gate A 证据要求
- IPC 证据：`backend/src/ipc/commands.rs` 对应命令与请求字段。
- 数据证据：`backend/src/storage/migrations/007/008/009_*`。
- 前端证据：`desktop/src/App.tsx` 导航与页面职责映射。

## Gate B（联调/闭环门）

### 入场条件（全部满足）
- [ ] Gate A 已通过。
- [ ] 前后端都按最新合同完成实现并回报变更清单。
- [ ] 运行环境已就绪（`HELPER_DB_*`、Notion 相关变量按模式配置）。

### 出场条件（全部满足）
- [ ] 构建通过：
  - [ ] `cd /Users/oliver/Documents/helper/desktop && pnpm build`
  - [ ] `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`
- [ ] 后端回归通过：
  - [ ] `cd /Users/oliver/Documents/helper/backend && cargo test -q`
- [ ] 发布域闭环通过：
  - [ ] `ingest_xhs_incremental -> review -> publish_approved_to_notion -> get_publish_history` 可追踪
  - [ ] 失败回放 `retry_dead_letters` 可恢复
- [ ] 异常路径通过：
  - [ ] 参数校验错误码为 `IPC-6001`
  - [ ] 存储失败错误码为 `DB-4001`
  - [ ] 内部异常错误码为 `INT-9000`
- [ ] 回滚路径可执行：
  - [ ] `NOTION_SYNC_MODE=database` 可用
  - [ ] 不改 IPC 命令名与 envelope

### B2 必测场景（Gate B 强制）

- [ ] B2-G1-01 目录结构：
  - Root -> Category -> Week -> Item 四层结构实际可见。
- [ ] B2-G1-02 默认分类路由：
  - 默认分类样本进入目标分类，不误入 unknown。
- [ ] B2-G1-03 低置信回退：
  - 低置信样本回退 unknown，`route_reason=low_confidence`。
- [ ] B2-G1-04 growth_limit 回退：
  - `category_growth_limit=0` 下自定义分类样本回退 unknown，`route_reason=growth_limit_exceeded`。
- [ ] B2-G1-05 Root Page 指向检查：
  - `/Users/oliver/Documents/helper/.ops/context/notion-root-page-checklist-v1.md` 全项通过。
- [ ] B2-G2-01 结构化成功样例：
  - `summary/key_points/tags/images/route_reason` schema 合法并成功写入。
- [ ] B2-G2-02 结构化降级样例：
  - `summary` 保留，降级字段为空且有 degraded 证据。
- [ ] B2-G2-03 结构化失败样例：
  - 非法 payload 返回受控错误，且可定位错误码。
- [ ] B2-G2-04 兼容窗口校验：
  - 2026-03-31 前旧字段仍可读取；B2 新实现不破坏旧数据读取。

### B2 证据格式（统一）

- [ ] 每个 B2 场景必须提供一条结构化证据记录，字段固定：
  - `scenario_id`
  - `owner`
  - `input`
  - `expected`
  - `actual`
  - `pass`
  - `artifacts`（截图路径/日志摘要/request_id）
- [ ] 证据载体固定：
  - `/Users/oliver/Documents/helper/.ops/context/b2-acceptance-template-v1.md`
- [ ] 不接受“口头通过”或“截图无上下文”的验收。

### Gate B 验收产物
- [ ] 前端验收记录（场景结果 + 关键截图）。
- [ ] 后端验收记录（关键命令输出摘要 + 计数）。
- [ ] 失败样本与修复结果（至少 1 条）。
- [ ] 本轮 residual risk 清单与下一步动作。
- [ ] B2 验收模板已填写并附证据链接。

## Fail-Close Rule

任一 Gate 未通过则视为“不可发布”：
- 不允许口头放行；
- 不允许跳过回滚条件；
- 必须先补齐缺项，再重新验收。
