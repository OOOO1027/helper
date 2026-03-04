# Backend Latest Status

State: GO (Guarded)

Date: 2026-02-26
Run time: 2026-02-26 05:07:25 CST

## Mandatory pre-read
- `master-spec.md`: read
- `decision-log.md`: read
- `backend-latest.md`: refreshed in this run
- `director-decisions.md`: read

## Tonight progress (minimum closed loop)
1. 采集入口规范化：
   - 增加 URL 识别与标准化、来源标注、时间戳归一、`url_hash`、`dedupe_key`。
2. 处理流水线：
   - 增加分类/标签/摘要/置信度输出；
   - 增加状态机（queued/processing/retry/failed/succeeded）；
   - 增加重试与失败队列判定。
3. Notion 适配：
   - 增加幂等 key 生成；
   - 增加 upsert plan（create/update/skip）；
   - 增加字段映射与错误返回结构。
4. 可观测性与错误码：
   - 统一 `BackendError -> code/retryable` 映射；
   - 入口添加结构化日志字段（最小集合）。
5. 本地持久化执行：
   - 新增 `execute_pipeline_with_conn`，将规范化/分析结果幂等写入
     `source_items/normalized_items/analysis_results/review_queue/sync_records`；
   - 存储异常自动写入 `dead_letter`（仅 storage/internal 类型）。
6. 测试：
   - 新增 normalize/pipeline/notion/app-core/DB 幂等关键用例。
7. 微信导入解析入口：
   - `txt/html` 可解析并抽取 URL、发布时间、dedupe_key；
   - `pdf/doc/docx` 已升级为本地工具最佳努力提取（失败回退 `PAR-2001`）；
   - 缺失文件返回 `ING-1001`；
   - `AppCore.import_wechat_files` 已接入解析结果统计（imported/duplicates/failed）。
8. IPC 持久化集成测试：
   - 新增 `run_pipeline_persist` 环境变量校验与跨调用幂等测试。
9. 微信端到端入库入口：
   - 新增 `import_wechat_and_persist_with_conn`（解析 -> 去重 -> pipeline -> SQLite）；
   - 新增 IPC `import_wechat_and_persist`（受控读取 `HELPER_DB_PATH/HELPER_DB_KEY`）；
   - 新增 E2E 测试覆盖“2成功+1重复+1失败”路径。
10. Notion 同步执行层：
   - 新增 `NotionHttpClient`（Query/Update/Create）；
   - 新增 `sync_pending_with_conn`（retry/backoff/dead_letter）；
   - 新增 IPC `run_notion_sync_once(limit)`；
   - 新增合同示例：`backend/contracts/ipc-sync-examples.json`。
   - 同步参数已配置化：`notion.sync.default_limit`、`notion.sync.sleep_ms`。
11. dead_letter 回放能力：
   - 新增 `retry_dead_letters_with_conn`；
   - 新增 IPC `retry_dead_letters(ids[])`；
   - 已扩展到 `pipeline/import` 单文件重放（本地路径可访问时自动恢复）；
   - `pipeline_execute` 支持 payload 回放（无路径也可重跑）；
   - 新增测试覆盖“sync_record/notion_sync”与“pipeline/wechat_import_parse”两类回放；
   - 新增回放审计字段（`replay_count/last_replayed_at/last_replay_status`）；
   - 新增审计增强字段（`last_replayed_by/last_replay_latency_ms`）。
12. 命令层环境保护测试：
   - `run_notion_sync_once` 缺失环境变量测试；
   - `retry_dead_letters` 缺失 DB 环境变量测试。
13. Notion 同步参数测试：
   - 新增 limit 生效测试（`limit=1` 时只处理 1 条 pending）。
14. 同步参数防护：
   - `limit` 自动钳制到 `[1,200]`，`sleep_ms` 自动钳制到 `[0,5000]`；
   - 非法值写入结构化 warn 日志；
   - 新增参数钳制单元测试。
15. 本地工具可用性 IPC：
   - 新增 `get_local_tool_status`（`textutil`/`pdftotext`）；
   - 合同示例已补齐到 `backend/contracts/ipc-sync-examples.json`。
16. 测试稳定性修复与回归：
   - 修复 `dead_letter_replay` 临时文件扩展名问题，回放测试恢复稳定；
   - 修复 `wechat_import_parse` 中依赖 `docx/pdf` 的环境敏感断言，改为明确不支持扩展名；
   - `cargo test -q` 全量通过。
17. Notion 冒烟回查能力：
   - 新增 `sync_notion::smoke`，执行 `sync_pending` 后按成功记录回查 Notion 页面并做一致性断言；
   - 新增可执行入口 `cargo run --bin notion_smoke`，输出结构化 JSON 报告并按结果返回退出码；
   - 新增 `notion_api` 页面快照解析与单测，回查比较逻辑单测；
   - `cargo check -q --bin notion_smoke` 与 `cargo test -q` 均通过。
18. 冒烟结果持久化：
   - 新增 `persist_smoke_report`，将冒烟摘要与明细写入 `job_runs.metadata_json`；
   - `notion_smoke` 二进制已接入该落库步骤，日志输出 `job_run_id`；
   - 新增 `persist_smoke_report_records_partial_job_run` 单测。
19. 同步日志接口实装：
   - 新增 `get_sync_logs_with_conn`，按时间范围分页读取 `sync_records + job_runs`；
   - IPC `get_sync_logs` 改为走 SQLCipher 真实读库路径；
   - 新增 `get_sync_logs` 环境变量校验测试与 DB 聚合读取测试；
   - 合同示例补充 `get_sync_logs` 请求/响应。
20. notion_sync_once 运行留痕：
   - `run_notion_sync_once` 执行后默认写入 `job_runs(job_type=notion_sync_once)`；
   - 状态映射：`success/partial/skipped/failed`，并记录 `success_count/fail_count/error_code`；
   - `get_sync_logs` 已扩展聚合 `notion_smoke + notion_sync_once`；
   - 新增对应单测并通过全量回归。
21. 日聚合视图（非破坏性）：
   - 新增 migration `005_sync_daily_stats_view.sql`，创建 `v_sync_daily_stats`；
   - 聚合维度：`day + job_type`，输出 `run_count/success_runs/partial_runs/failed_runs/success_rate/avg_success_count/avg_fail_count`；
   - 新增视图测试 `sync_daily_stats_view.rs` 并通过全量回归。
22. 日聚合 IPC 暴露：
   - 新增 `get_sync_daily_stats_with_conn`（分页读取 `v_sync_daily_stats`）；
   - 新增 IPC `get_sync_daily_stats(req.range, req.page)`；
   - 桌面端 `main.rs` 已注册该命令，前端 service/types 已增加对应调用封装；
   - 新增 DB 测试与 env 测试并通过全量回归。
23. 日聚合过滤参数补齐：
   - `get_sync_daily_stats` 请求新增可选 `job_type/status`；
   - `status` 过滤支持 `started/success/failed/partial/skipped`，非法值返回 `IPC-6001`；
   - 合同示例补充过滤请求与错误示例；
   - 新增 DB 测试覆盖 `job_type/status` 过滤与非法状态校验。
24. 日聚合边界值校验补齐：
   - 新增空时间范围（`range.from/range.to`）失败测试；
   - 新增分页非法（`page=0`）失败测试；
   - 合同错误样例补充 `range.from and range.to must not be empty`。
25. P0 真实环境冒烟验收执行（本轮）：
   - 环境预检查（shell-neutral）结果：`HELPER_DB_PATH/HELPER_DB_KEY/NOTION_TOKEN/NOTION_DATABASE_ID` 全部缺失；
   - 因 `BLOCKED_ENV` 触发，`cargo test -q` 与 `cargo run --bin notion_smoke` 未执行；
   - 本轮结论：未达到“可上线前验收通过”（阻塞类型：环境变量缺失）。
26. 演示态 -> 真实 DB 态（本轮完成）：
   - `collect`：不再返回固定 `30/25`，改为 `source_items` 按 source/时间窗口聚合；
   - `get_dashboard_metrics`：不再返回固定 `25/0.81/0.76`，改为 DB 统计；
   - `get_review_items`：不再固定单条 mock，改为 `review_queue` 过滤分页；
   - `update_review_item`：不再构造随机返回，改为真实更新并回读；
   - `retry_failed_items`：不再“全量 requeue”，改为按状态真实重试并统计 ignored。
27. 最小验证测试补齐（四类场景）：
   - 新增 `app_core_real_db_paths.rs`，覆盖空库、正常数据、失败重试、分页；
   - `cargo test -q` 全量通过。
28. 架构分层收敛（Clean/Hexagonal 最小落地）：
   - 新增端口层：`app_core/ports.rs`（`CoreDataPort`）；
   - 新增 SQLite 适配器层：`app_core/adapters/sqlite_core_port.rs`；
   - `app_core` 的 `collect/get_dashboard_metrics/get_review_items/update_review_item/retry_failed_items` 已改为通过端口调用适配器；
   - IPC 契约保持不变，`cargo test -q` 全量通过。
29. 小红书直连最小闭环（本轮完成）：
   - `collectors/xhs.rs` 从空实现升级为真实采集器：
     - 支持点赞 `/api/sns/web/v1/you/likes` 与收藏 `/web_api/sns/v1/file/faved/list`；
     - 支持分页、节流（默认 1.5s~3.0s，可由环境变量覆盖）；
     - 支持 Chrome 会话抓取（AppleScript JS）与 `XHS_COOKIE` 直连两种模式；
     - `HELPER_XHS_FAKE_ITEMS_JSON` 支持离线回归测试。
   - `app_core.collect_with_conn(source=xhs)` 改为真实入库路径：
     - 调用 xhs 采集器；
     - 执行 normalize + pipeline 持久化；
     - 遇已入库 fingerprint 停止后续入库；
     - 失败写入 `dead_letter(stage=xhs_collect)`。
   - 导入中心新增“一键同步小红书”按钮，调用既有 `collect` IPC（契约字段不变）。
   - 新增测试：
     - `collectors::xhs` 解析/时间归一/mock 入口测试；
     - `app_core_real_db_paths` 增加 xhs 增量停机与入库测试。
   - 验证结果：
     - `cd backend && cargo test -q` 通过；
     - `cd desktop && pnpm build` 通过。
30. 环境兼容与 AppleScript 降级路径（本轮完成）：
   - 新增 `backend/src/env_runtime.rs`：
     - 环境读取支持 `进程环境 -> login shell` 回退；
     - 支持 `bootstrap_env_from_shell` 启动引导；
     - 支持 `HELPER_DISABLE_SHELL_ENV_FALLBACK=1`（测试/诊断时禁用回退）。
   - `desktop/src-tauri/src/main.rs` 启动时自动引导：
     - `HELPER_DB_PATH/HELPER_DB_KEY/NOTION_TOKEN/NOTION_DATABASE_ID/XHS_COOKIE`。
   - xhs 采集器优先 cookie 模式：
     - `XHS_COOKIE`；
     - `XHS_COOKIE_FILE`（读取本地 cookie 文件内容）；
     - 若 cookie 不可用再尝试 AppleScript JS。
   - AppleScript 被禁用时错误提示改为双路径指引（开启 Chrome 开关或配置 cookie）。
   - IPC 层与 notion_smoke 环境读取也已接入 shell 回退，减少“App 内缺变量”误报。
31. Notion 页面树模式执行文档锁定（本轮更新文档，不含业务代码切换）：
   - `master-spec` 已新增 page_tree 目标结构、配置键、迁移与执行顺序；
   - `decision-log` 已新增 C 级决策留痕（用户已确认）；
   - 后续代码改造将以该文档为唯一执行基线，不再按旧 database-only 方案扩展。
32. Notion page_tree 第 1 阶段代码落地（schema/API/service/mode dispatch）：
   - `006_notion_page_tree.sql` 已注册并启用；
   - `notion_api` 已具备 `list_child_pages/create_child_page/move_page/append_blocks/update_paragraph_block`；
   - `service` 已新增 `sync_pending_page_tree_with_conn_with_options`，并写回 `sync_records.target_record_id=page_id`；
   - `run_notion_sync_once` 已支持 `NOTION_SYNC_MODE` 分派（默认 `page_tree`，`database` 保留回滚）；
   - 新增 env 测试：`page_tree` 模式缺 `NOTION_ROOT_PAGE_ID` 会显式失败；
   - `cd backend && cargo test -q` 全量通过。
33. Notion page_tree 集成测试收口（本轮）：
   - 新增 `NotionTreeClient` 端口与 `NotionHttpClient` 适配实现，支持 service 层 mock 注入；
   - 新增 `tests/notion_page_tree_service.rs`，覆盖：
     - 首次同步创建 `分类页 + 周页 + 内容页`；
     - 重复同步仅更新 block，不重复建内容页；
     - 分类迁移触发 `move_page` 且写入 `category_history_json`；
     - 重试耗尽后入 `dead_letter`；
   - 修复 page_tree item upsert 的 SQLite 冲突目标：
     - `ON CONFLICT(normalized_item_id)` -> `ON CONFLICT(id)`（避免 partial unique index 不可匹配）；
   - 回归结果：
     - `cargo test -q --test notion_page_tree_service` 通过（4/4）；
     - `cargo test -q` 全量通过。
34. 真实冒烟执行（本轮）：
   - 已在当前会话读取到 `HELPER_DB_PATH/HELPER_DB_KEY/NOTION_TOKEN/NOTION_ROOT_PAGE_ID`；
   - 执行 `cargo run --bin notion_smoke` 成功产出 `job_run_id=jr_9d70ec42-9e85-4ef8-87f4-818a7092c14a`；
   - 结果为 `mode=page_tree`，`attempted=0`，状态 `skipped`（暂无 pending 待同步数据）。
35. 冒烟输出可审计性增强（本轮）：
   - `notion_smoke` 终端 JSON 输出新增 `job_run_id`，结构为 `{ job_run_id, report }`；
   - 避免仅依赖 tracing 日志查找运行记录；
   - `cargo test -q` 全量回归通过。
36. Notion page_tree 稳定性收口（本轮）：
   - `notion.tree.timezone` 已真实生效：支持 `NOTION_TREE_TIMEZONE > notion.tree.timezone > Asia/Shanghai`；
   - 时区非法改为显式 `Validation` 错误，不再静默 fallback；
   - `notion.tree.category_growth_limit` 已真实生效：支持 `NOTION_TREE_CATEGORY_GROWTH_LIMIT > notion.tree.category_growth_limit > 0`；
   - 路由限流命中时写入 metadata：`route_reason: growth_limit_exceeded`。
37. Notion 只读配置快照 IPC（本轮）：
   - 新增 `get_notion_sync_config_snapshot`：
     - `sync_mode`
     - `notion_token_configured`
     - `notion_database_id_configured`
     - `notion_root_page_id_configured`
     - `timezone`
     - `category_growth_limit`
   - 桌面命令已注册并补 `NOTION_ROOT_PAGE_ID` 启动引导；
   - 测试新增并通过，`cargo test -q` 全量通过。
38. page_tree 专用冒烟闭环收口（本轮）：
   - `notion_smoke` 新增 `database/page_tree` 双模式分派，`report.mode` 可审计；
   - page_tree 冒烟回查项从数据库查询改为页面树实体回查：
     - 通过 `page_id` 回查页面标题；
     - 通过 `meta_block_id/summary_block_id` 回查段落内容；
   - 修复 `notion_smoke` 的 runtime 问题：
     - 原 `futures::executor::block_on` 在 reqwest 异步路径触发 `there is no reactor running`；
     - 改为 Tokio current-thread runtime 执行异步冒烟任务；
   - 回归：`cargo test -q` 全量通过。
39. 小红书采集 runtime 稳定性修复（本轮）：
   - `app_core.collect_xhs_with_conn` 同步入口已从 `futures::executor::block_on` 切换到 Tokio runtime；
   - 目的：避免桌面端触发 xhs 采集时出现同类 `there is no reactor running` 崩溃；
   - 回归：`cargo test -q` 全量通过。

## Current runnable path (code-level)
`wechat import/xhs sync -> normalize -> analyze -> queue decision -> sqlite persisted upsert -> notion sync`

## Gaps
1. 当前 Notion 冒烟为 `attempted=0`（暂无 pending 待同步数据），尚未形成“非零写入量”的 page_tree 验收样本。
2. 若未配置 cookie 且 Chrome 未开启 AppleScript JS，xhs 仍会失败（已提供替代路径：配置 `XHS_COOKIE` 或 `XHS_COOKIE_FILE`）。
3. 微信 `pdf/doc/docx` 提取依赖本机工具可用性（`textutil`/`pdftotext`），缺失时回退 `PAR-2001`。
4. `get_sync_logs/get_sync_daily_stats` 尚未切到与本轮一致的端口/适配器分层模式。

## Next smallest step
1. 先产出至少 1 条 pending（导入持久化或 xhs 一键同步），再执行 `cargo run --bin notion_smoke`。
2. 产出非零样本的 `job_run_id + attempted/succeeded/failed/mismatched` 并回写验收结论。
