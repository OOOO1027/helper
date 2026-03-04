# ADR-IA-v2

- Status: Accepted
- Date: 2026-02-26
- Owner: Product/Architecture + Frontend

## Context
现有桌面端需要把“业务处理动作”和“系统运维动作”明确拆开，保证用户在一个页面只做一类决策。  
同时必须明确边界：Notion 是最终呈现面，App 负责处理引擎，不在 Notion 承载运行态控制。

## Decision
采用 IA v2 导航与职责拆分，并固化边界：

1. 主导航固定为：`收件箱` / `审核` / `发布中心` / `系统健康`。
2. 页面职责互斥：
   - 收件箱：仅新增抓取与入箱结果（采集入口、入箱反馈）。
   - 审核：仅通过/驳回决策（低置信条目处理）。
   - 发布中心：仅 Notion 发布动作与发布日志（业务闭环动作）。
   - 系统健康：仅状态、配置快照、失败回放（运维动作）。
3. 任务页不再展示全局状态条；状态条仅在系统健康页显示。
4. v1 来源收敛为小红书点赞/收藏，微信/B站不作为主导航入口。
5. 边界声明：
   - Notion 最终呈现：只承载“发布后的内容结构与可读页面”。
   - App 处理引擎：负责采集、清洗、审核、发布队列、失败回放与观测。
   - 任何运行态字段（重试次数、错误码、死信状态）只在 App 呈现，不写入 Notion 交互面。

## Current Implementation Mapping (as-is)
以 `/Users/oliver/Documents/helper/desktop/src/App.tsx` 为当前实现基线：

1. `inbox -> InboxPage`
   - 主 IPC：`ingest_xhs_incremental`、`get_collection_insight`
   - 目标：完成“抓取 -> 入箱可见”。
2. `review -> ReviewQueuePage`
   - 主 IPC：`get_review_items`、`update_review_item`、`retry_failed_items`
   - 目标：完成“人工决策 -> 状态变更”。
3. `publish -> PublishCenterPage`
   - 主 IPC：`publish_approved_to_notion`、`get_publish_queue`、`get_publish_history`
   - 目标：完成“通过项 -> Notion 最终呈现”。
4. `health -> SystemHealthPage`
   - 主 IPC：`get_dashboard_metrics`、`get_collection_insight`、`get_notion_sync_config_snapshot`、`retry_dead_letters`、`get_publish_history`
   - 目标：完成“配置检查、失败定位、回放恢复”。

## Consequences

### Positive
1. 一屏一主任务，路径清晰。
2. 页面视觉层级更稳定，减少“面板拼贴感”。
3. 后续可以按页面边界并行开发，降低冲突。

### Negative
1. 旧页面仍在仓库中保留一段时间，短期存在并存成本。
2. 部分历史入口被迁移，需补充使用说明与回归测试。

## Acceptance Criteria
1. 导航只保留四个入口，名称与职责一致（`inbox/review/publish/health`）。
2. 收件箱/审核/发布中心页面不显示全局状态条；仅系统健康显示。
3. 发布动作只能从发布中心触发，失败恢复只能从系统健康触发。
4. Notion 相关“最终可见结果”通过发布中心反馈，不在审核页和系统健康页承载业务编辑。
5. `pnpm build` 通过。
