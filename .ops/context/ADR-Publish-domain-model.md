# ADR-Publish-domain-model

- Date: 2026-02-26
- Status: Accepted
- Deciders: Product + Frontend + Backend

## Context

当前发布链路如果只依赖 `sync_records`，会导致“业务发布状态”和“同步执行状态”耦合，前端无法稳定区分：
- 哪些内容待发布/已发布/失败；
- 哪些失败可回放；
- 哪些来源已被标记失效；
- 哪些结果是最终写入 Notion 的对外呈现。

## Decision

1. 新增发布域表：
- `publish_tasks`：发布队列状态（pending/processing/published/failed/ignored）。
- `notion_page_refs`：本地条目与 Notion 页面映射。
- `source_state`：来源是否失效（active/inactive）。
- `publish_audit`：发布审计记录（request_id、状态、耗时、错误）。

2. 新增 IPC：
- `ingest_xhs_incremental`
- `get_ingest_queue`
- `get_review_queue`
- `publish_approved_to_notion`
- `get_publish_queue`
- `get_publish_history`
- `mark_source_inactive`

3. 兼容策略：
- 旧 IPC（`collect/get_review_items/run_notion_sync_once`）保留一个迭代周期。
- 新 IPC 优先调用；旧运行时通过前端适配层回退。

4. 边界策略（本 ADR 强制）：
- Notion 最终呈现边界：
  - Notion 只承载最终可读内容（页面树或 database 回滚模式）。
  - Notion 不承载运行态调度信息（重试次数、死信状态、回放状态）。
- App 处理引擎边界：
  - App 本地 DB 承载队列、审核、发布状态、失败回放、审计日志。
  - 所有执行控制动作（发布、回放、失效标记）在 App 内完成，再同步到 Notion 最终面。

5. B2-G1 目录与分类规则（发布域必须对齐）：
- page_tree 目录固定：
  - `NOTION_ROOT_PAGE_ID` -> `category` -> `week(YYYY-Www)` -> `item`。
- 默认分类集合固定：
  - `学习|工作|生活|健康|财务|灵感|Inbox`。
- `category_growth_limit` 默认 `32`（允许自定义分类增长）；显式配置 `0` 时禁用扩类并回退 unknown。
- `category_growth_limit` 回退语义固定：
  - 低置信：`route_reason=low_confidence` -> unknown（默认 `Inbox`）。
  - 超增长上限：`route_reason=growth_limit_exceeded` -> unknown（默认 `Inbox`）。
- 参数非法（负数 growth_limit、非法 timezone）必须返回 `IPC-6001`，不允许静默修正。

6. B2-G2 结构化内容规则（发布域合并门禁）：
- 发布执行层结构化 schema 固定字段：`summary/key_points/tags/images/route_reason`。
- `summary` 必填；可降级字段为空时必须保留成功写入并标记 degraded。
- 兼容窗口：
  - 旧字段写入保留到 2026-03-31。
  - 2026-04-01 起新增变更未满足结构化 schema 则禁止合并。

## Current Implementation Mapping (as-is)

1. 数据层映射：
- `007_publish_domain.sql`：建表 `publish_tasks/notion_page_refs/source_state/publish_audit`。
- `008_publish_audit_observability.sql`：补 `pipeline_stage/sync_mode/retryable` 观测字段。
- `009_publish_audit_history_filters.sql`：补历史筛选索引。

2. 命令层映射（`backend/src/ipc/commands.rs`）：
- `publish_approved_to_notion(limit?)`：执行 Notion 发布并回写 `publish_tasks + publish_audit`。
- `get_publish_queue(page)`：读取发布队列主视图。
- `get_publish_history(range,page,state?,sync_mode?,retryable?)`：
  - 优先按 `publish_audit` 聚合；
  - 无审计数据时回退 `job_runs(job_type=notion_sync_once)`；
  - 回退路径不推断 `sync_mode/retryable`。
- `mark_source_inactive(item_id)`：同步更新 `source_state`、`publish_tasks`、`sync_records`。

3. 前端消费映射（`desktop/src/App.tsx`）：
- `PublishCenterPage`：触发发布、查看发布队列与发布历史。
- `SystemHealthPage`：查看配置快照、失败日志与 dead letter 回放。
- 由此确保“业务发布动作”和“系统恢复动作”分离。

## Executable Rules

1. 所有“是否应发布、是否可回放、是否来源失效”的判断以 App 本地 DB 为准，不以 Notion 页面反查作为判定源。
2. 前端不得绕过 `publish_approved_to_notion` 直接调用底层同步接口执行发布动作。
3. `get_publish_history` 的 legacy 回退仅在区间内 `publish_audit` 为空时生效，且仅支持 `state` 过滤。
4. 来源被标记 `inactive` 后，关联 `publish_tasks` 必须转 `ignored/source_inactive`，关联待同步记录必须转失败并写同错误码。
5. Notion 模式切换仅允许 `page_tree <-> database`，不新增第三种写入模型。
6. page_tree 路由参数必须遵循：`env > app_config > default`，且默认分类集合不得缺失 `Inbox`。

## Consequences

正向：
- 前端可按“收件箱/审核/发布中心/系统健康”稳定消费后端状态。
- 发布失败与重试路径有可审计记录。
- 取消收藏/来源失效后，可标记失效而不删除历史资产。

代价：
- 发布域与 `sync_records` 在过渡期并存，需要同步快照逻辑保持一致。
- 需在下一迭代将发布主路径逐步收敛到发布域，减少双写状态面。
