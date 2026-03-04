# Frontend Latest Status

State: GO (Guarded)

Date: 2026-02-26
Run time: 2026-02-26 16:26:00 CST
Run time (UTC): 2026-02-26T08:26:00Z

## Scope in this run
- 对齐“前端全量更新指南（业务闭环优先）”收口：
  - 导入中心保持“微信持久化导入 + XHS 一键同步”双入口，XHS 默认可见并补齐失败可操作引导。
  - 导入成功与 XHS 同步成功后增加一键跳转审核队列，压缩“导入/同步 -> 审核”路径。
  - 日统计增加“自定义状态”输入，可直接验证 `job_type/status` 非法值提示链路（`IPC-6001`）。
  - 错误映射补齐 XHS Chrome 权限类报错（Apple Events JavaScript 权限）用户文案。
  - 更新能力映射文档，补充 `collect(source)` 与验收路径说明。
- 同步与监控主线继续补强：
  - `RemoteState` 支持错误态一键重试，接入 Dashboard/Review/Settings。
  - `StatusStrip` 增加 60 秒自动刷新与请求防重入，避免顶栏监控数据陈旧。
  - 同步日志增加状态分布快捷筛选（chips）与筛选重置按钮。
  - 错误映射增强：补齐环境变量缺失、时间范围/分页/筛选参数异常的用户可读文案。
- 新增前后端能力映射文档：
  - `/.ops/context/frontend-backend-capability-map.md`
  - 覆盖四区页面 -> IPC 命令 -> 用户反馈路径，减少联调歧义。
- 设置页继续补强可观测主线：
  - 日统计顶部增加 7 天总览卡（总运行、成功率、单次平均成功、高频任务类型）；
  - 同步日志增加时间范围与状态筛选；
  - 日统计增加任务类型与运行状态筛选；
  - 新增日志与日统计分页控制（上一页/下一页/手动刷新）。
- 设置页主线补齐“同步可观测筛选”：
  - 同步日志增加时间范围（1/3/7/30 天）与状态筛选；
  - 日统计增加任务类型与运行状态筛选（通过 `get_sync_daily_stats` 参数下推）；
  - 同步/回放动作后继续统一刷新日志 + 日统计。
- 清理前端旧导入链路遗留：移除 `importWechatFiles` 与不再使用的 `ImportSummary/BackendImportSummary` 相关类型与 adapter。
- 完成 4 个 IPC 的 UI 接线闭环：`getLocalToolStatus` / `importWechatAndPersist` / `runNotionSyncOnce` / `retryDeadLetters`。
- 启动阶段接入本地工具探测，缺少 `textutil/pdftotext` 时应用内提示并在导入前硬拦截。
- 导入流程切换为 `importWechatAndPersist`，并通过适配层维持既有“总条数/有效URL/重复/失败”摘要结构。
- 设置页新增“手动同步一次”和“失败回放”入口，支持成功/失败反馈与日志刷新。
- 新增 `ImportPersistSummary -> ImportSummary` 字段兼容映射，继续统一 success/error 包络与用户文案映射。
- 建立 `desktop` 最小可运行工程骨架（Tauri + React + TypeScript）。
- 落地四区导航与主交互结构（列表 + 右侧详情抽屉）。
- 完成导入中心拖拽入口、导入摘要、错误反馈。
- 完成审核队列标签页（待审核/异常/已完成）与重试路径。
- 建立 IPC 稳定适配层，统一 loading/error/empty 三态与字段兼容映射。
- 增加导入后“前往异常标签页”一键跳转能力。
- 增加最近导入记录列表（时间/文件数/摘要/状态）。
- 增加审核列表标签化展示（优先级高/中/低视觉区分）。
- 接入 Tauri dialog 文件选择器（真实路径选择）并统一错误文案映射。
- 增加审核抽屉内 ID 复制与来源链接直达能力。
- 增加审核队列筛选/排序与本地持久化（localStorage）。
- 增加审核批量动作（通过/驳回/标记重复）与会话效率看板（条/分钟、15分钟预估、目标进度）。
- 增加筛选无结果时的一键清空筛选。
- 增加导入历史本地持久化（重启后可见最近记录）。
- 增加异常原因分组筛选（异常标签页按 reason 快速过滤）。
- 增加审核会话最近操作日志（最近 8 条动作记录）。
- 增加仪表盘“继续审核”直达入口（跳转审核队列待审核）。
- 使用 frontend-design 方向做视觉升级（Calm Ops Desk）：氛围背景、玻璃化侧栏、导航激活态强化、分层动效。
- 增加审核会话目标状态提示（达标/冲刺中）与目标差值显示。
- 增加异常分层筛选维度（按优先级 high/mid/low 组合过滤）。
- 增加批量操作失败明细面板（失败条目ID+错误信息）。
- 增加导入任务状态卡（准备中/待开始/导入中/已完成/失败）与可见进度条。
- 按 frontend-design 流程继续执行视觉升级：顶栏上下文骨架、仪表盘指标进度可视化、版式层级强化。

## Guarded constraints applied
- 微信文件导入作为主入口。
- B站默认关闭（仅设置页开关展示）。
- Notion 保持轻提示，不暴露复杂技术细节。
- 未引入付费依赖和未批准外部服务。
- 无破坏性操作。

## P0 / P1 task inventory
- P0 完成：前端工程从 0 到 1 建立并可进入联调。
- P0 完成：后端 IPC 命令对接适配（含错误包络处理）。
- P1 完成：组件复用（AppShell/StatusStrip/RemoteState/ReviewDrawer）。
- P1 完成：关键路径交互优化（审核动作后自动刷新、异常批量重试）。
- P1 完成：导入失败后可直达异常队列，降低路径跳转成本。
- P1 完成：真实文件路径选择能力（拖拽 + 系统文件选择器双通道）。
- P1 完成：联调阻塞修复（Rust 工具链就绪、Tauri 图标与后端构建错误已修复）。
- P1 完成：审核效率增强（筛选、排序、数量反馈、状态记忆）。
- P1 完成：批量审核操作与会话效率可视化（贴近日常 10-15 分钟目标）。
- P1 完成：异常定位效率增强（原因分组筛选 + 过程日志追踪）。
- P1 完成：首页到审核队列路径压缩（继续审核入口）。
- P1 完成：视觉一致性提升（Apple 骨架 + Notion 信息视觉更统一，未改业务逻辑）。
- P1 完成：异常处理链路可追溯增强（失败明细可见，过滤维度更完整）。
- P1 完成：导入反馈可见性增强（状态机 + 进度可视化）。
- P1 完成：视觉可读性增强（顶栏上下文 + 指标可视化 + 排版层级优化）。
- P1 完成：XHS 入口默认可见并提供受控失败引导（权限/登录指向下一步操作）。
- P1 完成：导入与 XHS 同步完成后可一键跳转审核队列。
- P1 完成：日统计筛选新增自定义状态输入，覆盖非法值提示验收路径。

## Checks
- Planned fastest checks:
  - `cd /Users/oliver/Documents/helper/desktop && pnpm install`
  - `cd /Users/oliver/Documents/helper/desktop && pnpm build`
  - `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`
- Executed checks:
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm build`（XHS/审核联动与统计自定义状态筛选改动后复验）
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`（桌面壳启动通过，四区主路径可点击）
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm build`（错误态重试/状态条刷新/筛选重置后复验）
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`（桌面壳启动通过）
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm build`（筛选+分页补强后复验）
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`（桌面壳启动通过）
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm build`（同步筛选与旧链路清理后复验）
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`（桌面壳启动通过）
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm build`（4 IPC UI 接线闭环后复验）
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`（桌面壳启动通过，联调入口可用）
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm install`
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm build`
  - ✅ `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`（编译通过并启动 `target/debug/helper-desktop`）
  - ✅ `source $HOME/.cargo/env && cargo -V && rustc -V`
  - ✅ `source $HOME/.cargo/env && cd /Users/oliver/Documents/helper/backend && cargo test`
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm build`
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm build`（原因筛选/会话日志/直达入口改动）
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm build`（frontend-design 视觉升级改动）
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`（桌面壳启动通过）
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm build`（目标状态提示改动）
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm build`（异常分层+失败明细改动）
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`（桌面壳启动通过）
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm build`（导入状态卡改动）
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`（桌面壳启动通过）
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm build`（顶栏上下文+指标可视化改动）
  - ✅ 最新增量后再次执行 `cd /Users/oliver/Documents/helper/desktop && pnpm tauri dev`（桌面壳启动通过）

## Build fixes applied during checks
- `backend/Cargo.toml`: 修正 `rusqlite` 无效 feature 名（`bundled-sqlcipher-vendored-openssl`）。
- `backend/src/ipc/commands.rs`: 修正 async tauri command 返回约束（同步 command + `block_on`）。
- `backend/src/pipeline/normalize.rs`: 修正 `and_then` 函数签名不匹配。
- `desktop/src-tauri/icons/icon.png`: 补齐 RGBA 图标文件，避免 `generate_context!` panic。
- `backend/tests/wechat_import_parse.rs` 与 `backend/tests/wechat_persist_flow.rs`: 修正临时文件命名，保留扩展名，恢复导入相关测试稳定性。

## Next smallest step
- 执行 20~50 条真实样本人工闭环验收（导入 -> 审核 -> 同步 -> 日志/统计 -> 回放），沉淀失败截图与日志样本。
