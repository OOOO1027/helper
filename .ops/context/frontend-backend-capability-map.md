# Frontend-Backend Capability Map

Status: ACTIVE
Owner: Frontend Lead
Last updated: 2026-02-26 16:20 CST

## Scope
- 目标：给前后端联调提供单一映射表，减少“页面该调哪个 IPC”歧义。
- 约束：仅记录已落地能力，不假设未声明字段。

## Dashboard（仪表盘）
- UI path:
  - `desktop/src/pages/DashboardPage.tsx`
  - `desktop/src/components/StatusStrip.tsx`
- IPC:
  - `get_dashboard_metrics`
  - `get_budget_status`
- Main feedback:
  - `RemoteState` loading/error/empty
  - 顶栏状态条仅显示轻量指标，不展示技术细节

## Import Center（导入中心）
- UI path:
  - `desktop/src/pages/ImportCenterPage.tsx`
- IPC:
  - `collect(source, since?, until?)`（XHS 一键同步，默认可见入口）
  - `get_local_tool_status`（启动提示 + 导入前硬拦截）
  - `import_wechat_and_persist`（主导入入口）
  - `pickWechatFiles`（Tauri dialog）
- Main feedback:
  - XHS 同步失败给可执行引导（Chrome Apple Events 权限 + 登录状态）
  - 导入状态机：`idle/ready/importing/success/failed`
  - 摘要字段：`total/parsed_ok/duplicates/parse_failed/persisted/persist_failed`
  - 缺工具提示明确到受影响文件类型（PDF/文档导出）
  - 导入/XHS 成功后可一键跳转审核队列

## Review Queue（审核队列）
- UI path:
  - `desktop/src/pages/ReviewQueuePage.tsx`
- IPC:
  - `get_review_items`
  - `update_review_item`
  - `retry_failed_items`
- Main feedback:
  - 标签页（待审核/异常/已完成）
  - 批量操作通知、失败明细、会话效率指标

## Settings & Logs（设置与日志）
- UI path:
  - `desktop/src/pages/SettingsLogsPage.tsx`
- IPC:
  - `run_notion_sync_once(limit)`
  - `retry_dead_letters(ids)`
  - `get_sync_logs(range, page)`
  - `get_sync_daily_stats(range, page, job_type?, status?)`
- Main feedback:
  - 同步动作期间统一禁用（按钮与输入）
  - 日志与日统计统一错误映射（`toUserMessage`）
  - 筛选项：
    - 日志：时间范围（1/3/7/30 天）+ 状态（前端筛选）
    - 日统计：任务类型 + 运行状态（后端聚合筛选）
    - 日统计支持“自定义状态”输入，用于联调 `IPC-6001` 非法值提示路径
  - 分页与手动刷新：日志/日统计均支持

## Shared IPC Rules
- Envelope:
  - 所有 IPC 走统一 `ApiResponse<T>` 包络。
  - `ok=false` 时统一通过 `IpcInvokeError` 抛出。
- User message:
  - 页面通知文案统一通过 `toUserMessage(error, fallback)` 映射。
- Runtime:
  - 非 Tauri 运行时使用 mock 返回，保证本地预览可运行。

## Known Constraints
- B站自动抓取默认关闭。
- 微信导入为主入口（每周一次手动导出 + 自动导入流程）。
- Notion 展示轻提示，不暴露 token/字段映射等细节。
