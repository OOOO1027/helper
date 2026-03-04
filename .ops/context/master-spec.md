# Master Spec (Provisional Baseline)

Status: DRAFT
Owner: Product/Architecture
Last updated: 2026-02-26 04:05:14 CST

## Purpose
- 提供前后端共享的单一架构基线，保证增量交付可审查、可回滚、可验证。

## Scope Boundary
- In scope: Web frontend, service backend, shared API contracts, data model evolution rules, delivery governance.
- Out of scope (until explicitly added): 新支付供应商接入、跨区域部署、新数据域重构。

## Non-Negotiable Delivery Rules
- 小步可审查变更：每次变更必须可单独评审与回滚。
- 合同优先：接口/事件/数据结构变更先更新本文件或 decision log，再实现代码。
- 兼容优先：默认向后兼容；若破坏兼容，必须先记录迁移方案和灰度计划。
- 安全基线：禁止泄露密钥/PII；日志必须脱敏。

## Interface Contract Baseline
- API contract source of truth:
  - `/Users/oliver/Documents/helper/backend/contracts/ipc-sync-examples.json`（IPC 行为样例）
  - `/Users/oliver/Documents/helper/desktop/src/types/contracts.ts`（前端类型契约）
- Frontend consumption rule: 仅依赖已发布合同字段；禁止假设未声明字段。
- Backend evolution rule: 新增字段优先可选；删除/重命名字段需先走 decision log。
- Error envelope baseline: 统一错误码、可观测 request id、用户可读 message（具体枚举待补齐）。

## Notion Sync Baseline (Locked)
- 目标：将 Notion 写入从「单数据库」切换为「页面树」模式。
- 目标结构：`Personal -> 分类页 -> 周归档页 -> 单条内容页`。
- 执行文档 source of truth：`/Users/oliver/Documents/helper/.ops/context/notion-page-tree-exec.md`
- 模式开关：
  - `NOTION_SYNC_MODE=page_tree`（默认）
  - `NOTION_SYNC_MODE=database`（仅回滚开关）
- 环境变量：
  - 必填：`NOTION_ROOT_PAGE_ID`
  - 继续保留：`NOTION_TOKEN`、`HELPER_DB_PATH`、`HELPER_DB_KEY`
  - `NOTION_DATABASE_ID` 仅在 `database` 模式读取
- 行为约束：
  - 不新增 IPC 命令名，不改变现有 envelope 结构
  - `run_notion_sync_once` 入口保持不变，仅切换内部策略
  - 错误码继续沿用 `IPC-6001 / DB-4001 / INT-9000`

## Notion Tree Defaults (app_config)
- `notion.sync.mode = page_tree`
- `notion.tree.timezone = Asia/Shanghai`
- `notion.tree.route_min_confidence = 0.72`
- `notion.tree.unknown_category = Inbox`
- `notion.tree.default_categories = 学习|工作|生活|健康|财务|灵感|Inbox`
- `notion.tree.category_growth_limit = 32`

## Notion Tree Data Model (Planned Migration)
- 新增 migration：`/Users/oliver/Documents/helper/backend/src/storage/migrations/006_notion_page_tree.sql`
- 新增表：`notion_tree_nodes`
- 关键约束：
  - `UNIQUE(node_type, parent_page_id, node_key)`
  - `UNIQUE(normalized_item_id) WHERE normalized_item_id IS NOT NULL`
  - `INDEX(node_type, parent_page_id)`

## Notion Tree Algorithm (Execution Order)
1. 先做 schema + Notion API 能力层。
2. 再做 service 的 `page_tree` 主流程。
3. 再做模式分派与配置读取。
4. 最后补测试与回归。

## Data and Migration Baseline
- 所有 schema 变更需具备前向与回滚策略。
- 写路径改动需定义幂等键或去重策略。
- 涉及异步处理时需定义重试上限和死信处理责任人。

## Quality Gates
- Fast checks first: lint/typecheck/unit（按改动最小集合执行）。
- 行为变更必须附带测试或明确说明风险豁免。
- 发布前必须存在可执行回退路径。

## Current Gaps To Resolve
- 缺少具体领域 API 清单与版本策略。
- 缺少前端路由/页面到后端能力的映射表。
- 缺少预算/配额监控阈值与告警动作定义。

## Next Update Trigger
- 任一接口合同变更
- 任一数据模型变更
- 任一发布策略调整
