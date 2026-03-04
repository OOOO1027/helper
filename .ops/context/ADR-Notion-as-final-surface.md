# ADR-Notion-as-final-surface

- Date: 2026-02-26
- Status: Accepted
- Deciders: Aiden (Director) + Product + Backend + Frontend

## Context

当前项目同时存在两类能力：
1. 处理引擎能力：采集、清洗、审核、发布队列、失败回放、预算守卫。
2. 最终呈现能力：把通过条目稳定展示到 Notion。

若两者边界不清，容易出现以下问题：
- 把运行态字段暴露到 Notion，导致用户侧信息噪音；
- 前端把“是否成功”建立在 Notion 页面即时可见上，弱化本地审计证据；
- 发布失败时无法用本地状态做可恢复闭环。

## Decision

1. Notion 定位为“最终呈现面（Final Surface）”：
- 只承载内容结果（页面树为默认模式，database 为回滚模式）。
- 不承载处理控制字段（重试次数、死信状态、回放审计）。

2. App 定位为“处理引擎（Processing Engine）”：
- 本地 DB 是流程真相源（source/review/publish/sync/dead_letter）。
- 发布、回放、失效标记、预算守卫都在 App 内完成并留痕。

3. 用户反馈链路：
- 业务动作结果先在 App 内反馈（状态、错误码、下一步）。
- Notion 用于最终内容浏览与消费，不用于流程控制。

4. B2-G1 路由与目录基线（必须执行）：
- 目录结构固定：`Root(NOTION_ROOT_PAGE_ID) -> Category -> Week(YYYY-Www) -> Item`。
- 默认分类集合固定：`学习|工作|生活|健康|财务|灵感|Inbox`。
- `route_min_confidence=0.72` 作为默认分流阈值。
- `category_growth_limit` 默认 `32`；显式设置为 `0` 时禁止新增自定义分类，统一回退 unknown（默认 `Inbox`）。
- 路由回退语义：
  - `low_confidence`：低置信直接回退 unknown。
  - `growth_limit_exceeded`：超增长上限回退 unknown。
- Root Page 检查必须通过：
  - `/Users/oliver/Documents/helper/.ops/context/notion-root-page-checklist-v1.md`

5. B2-G2 结构化内容基线（合并门禁）：
- 发布载荷必须具备结构化 schema：`summary/key_points/tags/images/route_reason`。
- `summary` 为必填；其余字段允许降级为空，但必须可观测标记为 degraded。
- 失败必须返回受控错误码并进入重试或死信，不允许静默丢弃。
- 兼容窗口固定到 2026-03-31：允许旧字段并存；2026-04-01 起新变更必须满足结构化 schema。

## Current Implementation Mapping (as-is)

1. 默认写入模式：
- `NOTION_SYNC_MODE=page_tree`；`database` 仅应急回滚。

2. 核心命令映射：
- `publish_approved_to_notion`：把通过项推送到 Notion，并写 `publish_audit`。
- `get_publish_history`：优先读 `publish_audit` 聚合；legacy 回退 `job_runs`。
- `run_notion_sync_once`：系统健康侧运维动作，不替代发布中心的业务发布入口。

3. 数据边界映射：
- Notion 页面映射落在 `notion_page_refs`。
- 页面树节点元数据落在 `notion_tree_nodes`（含 `category_name/week_key/meta_block_id/summary_block_id`）。
- 运行态和审计信息落在 `publish_tasks/source_state/publish_audit/sync_records/dead_letter`。

## Executable Rules

1. 前端不得把 Notion 当作流程状态查询主源，流程状态必须来自 IPC。
2. 任何新增“运行态字段”不得直接写入 Notion 页面结构。
3. 发布成功与否以 IPC 返回 + 本地审计落库为准，不以“Notion 页面是否立即打开”为准。
4. 发生发布异常时，恢复动作统一走 `retry_dead_letters`，不做手工绕过写入。
5. 需要回滚时仅切换 `NOTION_SYNC_MODE=database`，不改变 IPC 命令名与 envelope。

## Consequences

正向：
- 业务流程与内容呈现解耦，前后端联调边界清晰。
- 失败恢复可追踪、可重放、可回滚。

代价：
- 用户需要在 App 看运行态，在 Notion 看最终结果，交互入口是双面。

## Rollback

回滚触发条件（任一满足）：
1. page_tree 模式持续不可用且影响发布闭环；
2. 发布成功但最终内容错误率超过当日阈值且无法快速修复。

回滚动作：
1. 设置 `NOTION_SYNC_MODE=database`；
2. 保持 App 本地审计链路不变；
3. 在 `decision-log.md` 记录回滚开始/结束时间与影响范围。
