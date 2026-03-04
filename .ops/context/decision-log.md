# Decision Log

Status: ACTIVE
Owner: Product/Architecture + Delivery Director
Last updated: 2026-02-26 23:25:00 CST

## DL-2026-02-25-01
- Date: 2026-02-25
- Decision: 建立 `.ops/context/master-spec.md` 与 `.ops/context/decision-log.md` 作为强制 pre-read 基线文档。
- Rationale: 解除“缺失必读文件”导致的 P0 合同断裂，同时避免无合同开发。
- Impact:
  - Frontend/Backend 可执行 mandatory pre-read。
  - 实现任务仍需受限于“合同细节待补齐”状态，不自动放开所有开发。
- Owner: Product/Architecture
- Follow-up:
  1. 补齐 API contract 具体文件路径与版本策略。
  2. 补齐错误码与迁移策略细节。
  3. 在 frontend/backend 状态文档引用本 log 条目。

## DL-2026-02-25-02
- Date: 2026-02-25
- Decision: 对阻塞分级采用统一策略：
  - P0: 数据丢失/隐私风险/预算超支/核心合同断裂/超过两周期阻塞 => 立即暂停相关 track。
  - P1: 质量或排期退化 => 继续推进但附带缓解计划。
  - P2: 次要问题 => 记录并按计划修复。
- Rationale: 保证跨团队升级路径一致，减少执行歧义。
- Impact: Director、Frontend、Backend 文档均需按同一分级语言更新。
- Owner: Delivery Director
- Follow-up: 下次 run 核验三份状态文档是否一致使用该分级语言。

## DL-2026-02-25-03
- Date: 2026-02-25
- Decision: 后端执行链路采用“最短闭环增强”方案：
  - 保持现有 schema 不做破坏性调整；
  - 新增采集规范化输出字段（`extracted_urls/url_hash/dedupe_key`）；
  - 新增处理状态机与失败队列判定；
  - Notion 侧先实现幂等 upsert 计划与字段映射，不接入新外部服务。
- Decision level: B（并发/批量参数未调；涉及非关键字段扩展，已留痕）
- Candidate scoring:
  - 方案 A（保持骨架，仅补文档）= 可靠性 62 / 效率 88 / 成本 95 / 维护 72 => 总分 76.85
  - 方案 B（最短闭环增强）= 可靠性 84 / 效率 80 / 成本 92 / 维护 83 => 总分 84.25
  - 分差 7.4（<10），按规则选“依赖更少、回滚更容易、成本更低”的方案 B。
- Rationale: 满足今晚目标中的“可跑通链路 + 幂等 + 重试 + 失败队列 + 最小可观测”，且不触发 C 级条件。
- Impact:
  - 可在本地无外部调用情况下验证处理链路输入/输出与错误路径。
  - 不改变主模型供应商、不新增付费 API、不变更 Notion 核心库结构。
- Owner: Backend Lead
- Follow-up:
  1. 接入真实 DB 持久化执行（目前为可测试骨架）。
  2. 接入真实 Notion API 客户端并复用当前幂等计划结构。

## DL-2026-02-25-04
- Date: 2026-02-25
- Decision: 在不引入新外部服务前提下，新增本地持久化执行入口与 IPC 预览接口：
  - `execute_pipeline_with_conn`：幂等写入 `source_items/normalized_items/analysis_results/review_queue/sync_records`；
  - storage/internal 错误进入 `dead_letter`；
  - IPC 新增 `run_pipeline_preview`，用于前后端联调验证输出结构。
- Decision level: B（接口契约扩展，非破坏性）
- Rationale: 缩短“采集到落库”的验证路径，降低联调风险与回滚复杂度。
- Impact:
  - 保持单机、低成本、可回滚目标；
  - 不涉及付费 API、不改变 Notion 核心库结构。
- Owner: Backend Lead
- Follow-up:
  1. 在合同文件中补充 `run_pipeline_preview` 响应示例。
  2. 将持久化执行入口以受控 IPC 方式暴露（需密钥从环境变量注入）。

## DL-2026-02-25-05
- Date: 2026-02-25
- Decision: 新增“微信导入直达持久化”入口，保持单机链路闭环：
  - `import_wechat_and_persist_with_conn`（AppCore）
  - `import_wechat_and_persist`（IPC）
  - 微信解析器支持 `txt/html`，对 `pdf/doc/docx` 返回受控错误码。
- Decision level: B（接口契约扩展 + 非关键参数演进）
- Candidate scoring:
  - 方案 A（仅保留现有 preview/persist 单条入口）= 可靠性 74 / 效率 70 / 成本 92 / 维护 75 => 总分 76.8
  - 方案 B（新增批量导入持久化入口）= 可靠性 86 / 效率 82 / 成本 90 / 维护 83 => 总分 85.3
  - 分差 8.5（<10），按规则选择依赖更少且回滚简单的方案 B（沿用现有 DB/错误码，不新增外部依赖）。
- Rationale: 满足“先打通最短闭环”与“幂等可恢复”目标，减少前后端联调拼装成本。
- Impact:
  - 导入链路可一次调用完成解析、去重、入库与失败留档。
  - 不新增付费 API，不变更主模型供应商，不修改 Notion 核心库结构。
- Owner: Backend Lead
- Follow-up:
  1. 补充 IPC 合同示例（请求/响应/错误码）。
  2. 后续在真实 Notion client 接入后复用当前 `sync_records` 与 dead_letter 机制。

## DL-2026-02-25-06
- Date: 2026-02-25
- Decision: 接入 Notion 同步执行服务与单次触发 IPC：
  - 新增 `NotionHttpClient`（基于 `NOTION_TOKEN` + `NOTION_DATABASE_ID`）；
  - 新增 `sync_pending_with_conn`（重试退避、状态回写、dead_letter）；
  - 新增 IPC `run_notion_sync_once(limit)`。
- Decision level: B（并发/批量参数与接口能力扩展，非破坏性）
- Candidate scoring:
  - 方案 A（仅保留计划层，不执行真实同步）= 可靠性 68 / 效率 76 / 成本 96 / 维护 79 => 总分 76.55
  - 方案 B（增加受控单次同步执行）= 可靠性 88 / 效率 79 / 成本 91 / 维护 82 => 总分 85.00
  - 分差 8.45（<10），按规则选依赖更少且回滚简单方案 B（单次触发、环境变量注入、无新第三方）。
- Rationale: 在不引入新付费服务前提下，完成“写入终点 Notion”的可执行闭环。
- Impact:
  - 同步失败可重试，耗尽后进入 dead_letter，可回放；
  - 新增 IPC 合同示例，前后端联调可直接对齐。
- Owner: Backend Lead
- Follow-up:
  1. 批量策略与并发度后续按真实数据量压测再调参。
  2. 增加端到端回放命令（dead_letter -> retry）。

## DL-2026-02-25-07
- Date: 2026-02-25
- Decision: 新增 dead_letter 回放能力：
  - `retry_dead_letters_with_conn`（AppCore）
  - IPC `retry_dead_letters(ids[])`
  - 仅对 `entity_type=sync_record` 且 `stage=notion_sync` 的 open 项自动回放。
- Decision level: A（错误处理与恢复策略增强）
- Rationale: 提供最小可恢复路径，避免同步失败长期堆积。
- Impact:
  - 回放后 `sync_records` 进入 `retry`，对应 dead_letter 标记 `resolved`；
  - 非匹配类型保持忽略，不做破坏性修改。
- Owner: Backend Lead
- Follow-up:
  1. 后续支持更多 stage 的自动回放（pipeline/import）。
  2. 增加回放审计日志导出能力。

## DL-2026-02-25-08
- Date: 2026-02-25
- Decision: 扩展 dead_letter 回放到 pipeline/import 最小可执行场景：
  - `entity_type=pipeline` 且 `stage in (wechat_import_parse, wechat_persist, pipeline_execute)` 时，
    若 `entity_id` 为本地可访问文件路径，则自动重放“单文件解析 -> pipeline 持久化”。
- Decision level: A（恢复机制增强，非破坏性）
- Rationale: 降低人工重放成本，缩短异常恢复时间。
- Impact:
  - 对可重放路径自动恢复并将 dead_letter 标记 resolved；
  - 非可访问路径保持 open + ignored，不做危险操作。
- Owner: Backend Lead
- Follow-up:
  1. 补充回放审计字段（谁触发、重放耗时、重放结果）。
  2. 扩展到批量路径恢复与冲突去重统计。

## DL-2026-02-25-09
- Date: 2026-02-25
- Decision: Notion 同步参数改为“请求参数 + app_config 默认值”双通道：
  - `notion.sync.default_limit`（默认 50）
  - `notion.sync.sleep_ms`（默认 400）
  - IPC `run_notion_sync_once(limit)` 的 `limit` 优先级高于配置默认值。
- Decision level: B（批量/节流参数调优）
- Candidate scoring:
  - 方案 A（继续硬编码 50/400）= 可靠性 78 / 效率 70 / 成本 88 / 维护 66 => 总分 75.9
  - 方案 B（配置化可调）= 可靠性 85 / 效率 80 / 成本 90 / 维护 84 => 总分 84.2
  - 分差 8.3（<10），按规则选依赖更少、回滚容易、成本更低的方案 B（仅本地配置，不加新依赖）。
- Rationale: 便于后续压测与成本控制，避免频繁改代码发布。
- Impact:
  - 同步速度可按数据规模和限流表现在线调优；
  - 保持默认安全阈值，兼容现有调用方。
- Owner: Backend Lead
- Follow-up:
  1. 增加配置项校验与非法值告警。
  2. 在压测后回写推荐参数区间。

## DL-2026-02-25-10
- Date: 2026-02-25
- Decision: 为 dead_letter 回放增加审计字段与写入逻辑：
  - `replay_count`
  - `last_replayed_at`
  - `last_replay_status`
- Decision level: A（可观测性与恢复机制增强）
- Rationale: 满足“可恢复、可追溯”要求，便于排障与运营复盘。
- Impact:
  - 每次回放都会记录结果（resolved/ignored）与次数；
  - 非破坏性 schema 扩展（新增 migration）。
- Owner: Backend Lead
- Follow-up:
  1. 在 dashboard 指标中加入 dead_letter 回放成功率。
  2. 为长尾失败项增加自动升级规则。

## DL-2026-02-25-11
- Date: 2026-02-25
- Decision: 为 Notion 同步配置增加运行时保护：
  - `limit` 钳制到 `[1, 200]`
  - `sleep_ms` 钳制到 `[0, 5000]`
  - 非法值触发结构化 warn 日志。
- Decision level: A（错误处理与稳定性增强）
- Rationale: 防止配置误填导致速率过高或任务停滞。
- Impact:
  - 保持配置灵活性，同时限制极端参数风险；
  - 新增单元测试覆盖参数钳制逻辑。
- Owner: Backend Lead
- Follow-up:
  1. 后续把钳制阈值迁移到可配置白名单。
  2. 增加告警统计（clamp 次数/日）。

## DL-2026-02-25-12
- Date: 2026-02-25
- Decision: 扩展 dead_letter 回放审计字段为“触发者 + 耗时”：
  - 新增 `last_replayed_by`
  - 新增 `last_replay_latency_ms`
  - 默认触发者来自 `HELPER_OPERATOR`，缺省为 `system`。
- Decision level: A（可观测性增强）
- Rationale: 满足异常恢复过程的责任追踪和性能分析。
- Impact:
  - 回放结果可按人/耗时聚合；
  - 非破坏性 schema 扩展（新增 migration）。
- Owner: Backend Lead
- Follow-up:
  1. 在 dashboard 增加回放耗时分位统计。
  2. 增加按 operator 过滤的审计查询接口。

## DL-2026-02-25-13
- Date: 2026-02-25
- Decision: dead_letter 支持 payload 驱动回放：
  - 对 `pipeline_execute` 优先反序列化 `payload_json` 为请求体并直接重跑；
  - 对导入类失败支持 `payload_json.path` 回放路径兜底。
- Decision level: A（恢复机制增强）
- Rationale: 解决 entity_id 不可直接重放时（如 `unknown`）的恢复缺口。
- Impact:
  - 回放能力不再强依赖文件路径；
  - 兼容旧数据（无 payload 时仍走原有逻辑）。
- Owner: Backend Lead
- Follow-up:
  1. 为 payload 增加 schema 版本字段。
  2. 增加 payload 合法性校验错误码。

## DL-2026-02-25-14
- Date: 2026-02-25
- Decision: 微信解析器新增本地工具最佳努力提取：
  - `doc/docx` 使用 `textutil -convert txt -stdout`
  - `pdf` 使用 `pdftotext <file> -`
  - 提取失败时保持 `PAR-2001` 受控失败。
- Decision level: A（解析器优化）
- Rationale: 在不引入外部服务和额外成本的前提下提升可解析覆盖面。
- Impact:
  - 支持能力取决于本机工具可用性；
  - 不影响既有失败回退路径。
- Owner: Backend Lead
- Follow-up:
  1. 增加工具可用性自检接口与提示文案。
  2. 后续评估统一文档解析库替代系统命令。

## DL-2026-02-25-15
- Date: 2026-02-25
- Decision: 修复两处测试稳定性问题并以全量回归作为当晚基线：
  - `tests/dead_letter_replay.rs`：临时文件生成保持扩展名，避免回放测试误判为不支持扩展名；
  - `tests/wechat_import_parse.rs`：将“必失败样例”改为明确不支持扩展名，避免 `docx/pdf` 在不同机器上结果漂移；
  - 执行 `cargo test -q`，全量测试通过。
- Decision level: A（测试补齐与可靠性修复）
- Rationale: 解除“环境相关测试波动”导致的假失败，确保 CI/本地一致性。
- Impact:
  - 提升回放链路与导入解析链路的可验证性；
  - 文档中“cargo 不可用”的陈旧信息已同步清理。
- Owner: Backend Lead
- Follow-up:
  1. 增加真实 Notion 环境冒烟脚本并固化回查断言。
  2. 将工具可用性检查接入启动期健康检查清单。

## DL-2026-02-26-16
- Date: 2026-02-26
- Decision: 新增 Notion 同步后回查冒烟工具并接入可执行二进制：
  - 新增 `sync_notion::smoke::run_notion_sync_smoke_with_conn`（先执行 `sync_pending`，再对成功记录回查 Notion 页面）；
  - 新增 `src/bin/notion_smoke.rs`，支持 `cargo run --bin notion_smoke` 一键输出 JSON 报告；
  - 冒烟断言覆盖 `normalized_item_id/title/summary/source/source_url` 一致性。
- Decision level: A（可观测性、错误处理、测试补齐）
- Rationale: 把“真实写入 + 回查一致性”从手工操作收敛为可重复执行命令，降低验收成本。
- Impact:
  - 线上冒烟从“只看 sync_state”升级为“写入后字段一致性验证”；
  - 不引入新付费服务，不改 Notion 核心库结构，不做破坏性变更。
- Owner: Backend Lead
- Follow-up:
  1. 在真实密钥环境执行一次并归档报告。
  2. 后续将报告摘要写入 `job_runs.metadata_json`。

## DL-2026-02-26-17
- Date: 2026-02-26
- Decision: 冒烟结果默认写入 `job_runs` 历史表：
  - 新增 `persist_smoke_report`；
  - `notion_smoke` 执行完成后自动落库（`job_type=notion_smoke`）；
  - 失败/不一致通过 `status=partial|skipped` 与 `error_code/error_message` 标识。
- Decision level: A（可观测性与错误处理增强）
- Rationale: 让线上验收结果可追踪、可审计，避免仅依赖终端输出。
- Impact:
  - 保留单机架构，不新增外部依赖；
  - 不改动业务主流程，只增强运行证据链。
- Owner: Backend Lead
- Follow-up:
  1. 后续把 `notion_smoke` 历史分页查询接入 `get_sync_logs`。
  2. 增加按日成功率统计视图。

## DL-2026-02-26-18
- Date: 2026-02-26
- Decision: `get_sync_logs` 从占位返回切换为真实读库：
  - 增加 `AppCore::get_sync_logs_with_conn`，分页读取 `sync_records` 与 `job_runs(job_type=notion_smoke)`；
  - IPC `get_sync_logs` 统一走 SQLCipher（需 `HELPER_DB_PATH/HELPER_DB_KEY`）；
  - 增加环境变量缺失测试与 DB 聚合读取测试。
- Decision level: A（可观测性、错误处理、测试补齐）
- Rationale: 让“同步日志面板”依赖真实数据，支撑运维和回放定位。
- Impact:
  - 前后端联调可直接消费真实分页数据；
  - 无破坏性 schema 变更，无新增外部依赖。
- Owner: Backend Lead
- Follow-up:
  1. 增加 `notion_smoke` 按日聚合统计接口。
  2. 增加状态过滤参数（pending/failed/partial/success）。

## DL-2026-02-26-19
- Date: 2026-02-26
- Decision: `run_notion_sync_once` 增加运行历史落库：
  - 同步执行后自动写入 `job_runs(job_type=notion_sync_once)`；
  - 记录 `status/success_count/fail_count/error_code/error_message/metadata_json(limit,sleep_ms,summary)`；
  - `get_sync_logs` 聚合范围扩展到 `notion_smoke + notion_sync_once`。
- Decision level: A（可观测性与错误处理增强）
- Rationale: 把同步执行从“只返回一次结果”升级为“可追溯历史事件”。
- Impact:
  - 后端可追踪每次同步运行质量和失败码；
  - 不涉及 schema 破坏性变更，不新增外部依赖。
- Owner: Backend Lead
- Follow-up:
  1. 提供按日聚合统计接口（成功率、平均失败率）。
  2. 增加状态过滤参数供前端面板使用。

## DL-2026-02-26-20
- Date: 2026-02-26
- Decision: 增加 `v_sync_daily_stats` 聚合视图（migration 005）：
  - 基于 `job_runs` 聚合 `notion_smoke + notion_sync_once`；
  - 输出按日成功率与平均成功/失败计数；
  - 附带视图单测，确保迁移后可查询。
- Decision level: A（非破坏性 schema 扩展、可观测性增强）
- Rationale: 在不改 IPC 的前提下先落地统计底座，降低后续联调改动风险。
- Impact:
  - 数据层具备日级统计能力；
  - 现有业务流程无行为变化。
- Owner: Backend Lead
- Follow-up:
  1. 将视图结果通过 IPC 暴露给前端仪表盘。
  2. 增加 job_type/status 过滤参数。

## DL-2026-02-26-21
- Date: 2026-02-26
- Decision: 把 `v_sync_daily_stats` 通过 IPC 正式暴露：
  - 新增 `AppCore::get_sync_daily_stats_with_conn`；
  - 新增 IPC `get_sync_daily_stats(req.range, req.page)`；
  - 桌面端注册命令并补前端 service/types 封装，保持合同一致。
- Decision level: A（可观测性增强、非破坏性接口扩展、测试补齐）
- Rationale: 缩短“统计视图到前端面板”的路径，避免后续联调改动面扩大。
- Impact:
  - 前端可直接分页读取日聚合统计；
  - 不改动历史数据，不引入外部依赖。
- Owner: Backend Lead
- Follow-up:
  1. 增加 `job_type/status` 过滤能力。
  2. 增加统计接口的边界值测试（空区间/分页越界）。

## DL-2026-02-26-22
- Date: 2026-02-26
- Decision: 为 `get_sync_daily_stats` 增加可选过滤参数并对齐合同：
  - IPC 请求扩展 `job_type/status`（均为可选字段）；
  - `AppCore::get_sync_daily_stats_with_conn` 增加过滤实现；
  - `status` 过滤按 `job_runs.status` 存在性匹配（`started/success/failed/partial/skipped`）；
  - 合同示例与测试同步更新。
- Decision level: A（错误处理、查询能力与测试补齐）
- Rationale: 解决统计面板缺少筛选条件导致的联调返工风险，同时保持非破坏性扩展。
- Impact:
  - 旧调用方无须改动（字段可选）；
  - 新调用方可按任务类型和状态筛选日聚合结果。
- Owner: Backend Lead
- Follow-up:
  1. 增加 `get_sync_daily_stats` 的分页越界/空区间边界测试。
  2. 前端面板接入过滤项并验证错误码映射。

## DL-2026-02-26-23
- Date: 2026-02-26
- Decision: 为 `get_sync_daily_stats` 补齐边界值测试并更新合同错误样例：
  - 新增空时间范围校验测试；
  - 新增分页参数非法（`page=0`）校验测试；
  - 合同新增 `range.from and range.to must not be empty` 错误样例。
- Decision level: A（测试补齐、错误处理增强）
- Rationale: 提前锁定边界行为，避免前端在异常分支上返工。
- Impact:
  - 错误路径可回归且具备合同样例；
  - 无 schema 变更、无外部依赖变更。
- Owner: Backend Lead
- Follow-up:
  1. 前端在调用层统一映射 `IPC-6001` 的 range/page/status 三类提示。

## DL-2026-02-26-24
- Date: 2026-02-26
- Decision: 将 5 个核心 IPC 所依赖的演示返回切换为真实 DB 路径（不改字段契约）：
  - `collect` -> 基于 `source_items` 按 source/time window 聚合；
  - `get_dashboard_metrics` -> 基于 `source_items/review_queue/analysis_results/budget_ledger` 统计；
  - `get_review_items` -> 基于 `review_queue` 过滤分页；
  - `update_review_item` -> 更新 `review_queue` 并回读；
  - `retry_failed_items` -> 将非 pending 项回置 `pending` 并统计 ignored/requeued。
- Decision level: A（非破坏性实现替换、错误处理与测试补齐）
- Rationale: 把“演示态”拉到“真实读写态”，降低前端联调返工风险。
- Impact:
  - IPC 字段名保持不变，前端无需改 contract；
  - DB 环境变量缺失时以上接口将返回 `IPC-6001`（与现有语义一致）。
- Owner: Backend Lead
- Follow-up:
  1. 在真实 DB 环境完成一次从导入持久化到仪表盘/审核队列读取的联调验收截图。
  2. 继续推进 P0 Notion 冒烟（补齐环境变量后执行）。

## DL-2026-02-26-25
- Date: 2026-02-26
- Decision: 按 Clean/Hexagonal 最小落地，将 `app_core` 的数据访问职责下沉为“端口 + SQLite 适配器”：
  - 新增端口 `app_core::ports::CoreDataPort`；
  - 新增适配器 `app_core::adapters::sqlite_core_port::SqliteCoreDataPort`；
  - `collect/get_dashboard_metrics/get_review_items/update_review_item/retry_failed_items` 的 `*_with_conn` 路径统一经端口调用。
- Decision level: A（非破坏性分层重构、可测试性增强）
- Rationale: 在不改变 IPC 契约前提下，收敛 `app_core` 职责边界，减少 SQL 与业务编排耦合。
- Impact:
  - IPC 字段名和错误包络保持不变；
  - SQL 细节集中到 adapter，`app_core` 聚焦校验和流程编排；
  - 后续可用 mock port 替换 SQLite adapter 做 use-case 级测试。
- Owner: Backend Lead
- Follow-up:
  1. 为 `CoreDataPort` 增加 in-memory/mock adapter，覆盖无 DB 的用例测试。
  2. 将同样模式推广到 `get_sync_logs` 与 `get_sync_daily_stats` 读路径。

## DL-2026-02-26-26
- Date: 2026-02-26
- Decision: 小红书采集先采用“Chrome 已登录会话 + 受控节流 + 本地 DB 闭环”最小方案：
  - `collectors/xhs.rs` 支持点赞/收藏分页抓取；
  - 抓取优先使用 Chrome 会话（AppleScript JS），可选 `XHS_COOKIE` 直连；
  - `app_core.collect(source=xhs)` 执行真实 pipeline 入库，遇已入库 fingerprint 停止后续入库；
  - 失败落 `dead_letter(stage=xhs_collect)`，不改 IPC 字段契约。
- Decision level: B（采集与节流参数引入，非关键字段/策略扩展，已留痕）
- Candidate scoring:
  - 方案 A（继续微信导入中转）= 可靠性 73 / 效率 62 / 成本 92 / 维护 84 => 总分 75.70
  - 方案 B（Chrome 会话直连最小闭环）= 可靠性 82 / 效率 86 / 成本 88 / 维护 79 => 总分 83.05
  - 分差 7.35（<10），按规则选择依赖更少、回滚更容易、成本更低的方案 B（本地单体、无新增付费服务）。
- Rationale: 先把“可阶段验收”从演示态拉到真实读写态，满足用户对“收藏/点赞直接处理”的最短闭环诉求。
- Impact:
  - 前端可直接点击“一键同步小红书”触发后端真实入库；
  - 若 Chrome 未开启 AppleScript JS，会返回可操作错误，不会静默失败；
  - 未触发 C 级变更（无新增付费 API、无 Notion 核心库结构变更、无破坏性删改）。
- Owner: Backend Lead
- Follow-up:
  1. 在桌面端记录一次真实小红书同步 run 证据（计数与日志）。
  2. 完成 Notion 冒烟后再打通“xhs 入库 -> notion 同步”端到端闭环。

## DL-2026-02-26-27
- Date: 2026-02-26
- Decision: 增加“环境变量 login shell 回退 + cookie 优先”稳定性策略，降低 GUI 启动与 AppleScript 限制导致的失败率：
  - 新增 `env_runtime`，支持 `env -> login shell` 回退读取；
  - Desktop 启动时对 `HELPER_DB_* / NOTION_* / XHS_COOKIE` 执行一次环境引导；
  - xhs 采集优先走 `XHS_COOKIE`（含 `XHS_COOKIE_FILE`），仅在 cookie 不可用时回退 AppleScript JS；
  - AppleScript 被禁用时返回可操作错误提示（含 cookie 替代路径）。
- Decision level: A（错误处理、稳定性与可运维性增强）
- Rationale: 解决“桌面端提示本地存储未就绪”和“未开启 AppleScript JS 导致 xhs 直连失败”的主要体验阻塞。
- Impact:
  - 若变量已配置在 shell profile，GUI 启动也可读取；
  - 用户可不依赖 AppleScript JS，改用 cookie 模式完成同步；
  - IPC 契约与字段保持不变。
- Owner: Backend Lead
- Follow-up:
  1. 后续将敏感配置迁移到 Keychain，减少环境变量依赖。
  2. 增加“当前采集模式（cookie/applescript）”可观测字段用于排障。

## DL-2026-02-26-28
- Date: 2026-02-26
- Decision: Notion 同步主路径切换为「页面树模式（page_tree）」并保留 database 回滚模式：
  - 结构固定为 `Personal -> 分类页 -> 周归档页 -> 单条内容页`；
  - `run_notion_sync_once` IPC 入口不变，仅内部模式分派；
  - 新增 `NOTION_ROOT_PAGE_ID`，`NOTION_SYNC_MODE` 默认 `page_tree`；
  - 新增迁移 `006_notion_page_tree.sql` 与 `notion_tree_nodes` 映射表；
  - 分类策略固定为“预置清单 + 自动扩展”，低置信进入 `Inbox`；
  - 同步失败重试 3 次后进入 `dead_letter`（沿用现有框架）。
- Decision level: C（修改 Notion 核心写入结构，已在当前 thread 获用户明确确认）
- Rationale: 满足“同一内容去重更新、可迁移分类、可周归档、可审计回滚”的产品目标，同时避免前端重复返工。
- Impact:
  - Notion 写入目标从数据库属性建模改为页面树建模；
  - 既有 IPC 契约与错误码语义不变；
  - `database` 模式保留为应急回滚路径。
- Owner: Backend Lead
- Follow-up:
  1. 按顺序执行：schema/API -> service -> mode dispatch -> tests。
  2. 回归要求：`cargo test -q` 与 `run_notion_sync_once(page_tree)` 可执行。

## DL-2026-02-26-29
- Date: 2026-02-26
- Decision: 完成 page_tree 第一阶段“可执行收口”，采用端口/适配器最小边界实现并保持 IPC 契约不变：
  - `sync_notion/service` 补齐 page_tree 主流程与成功写回 `target_record_id=page_id`；
  - `ipc::run_notion_sync_once` 增加 `NOTION_SYNC_MODE` 分派（默认 `page_tree`，`database` 兼容回滚）；
  - `page_tree` 模式改为依赖 `NOTION_ROOT_PAGE_ID`，`NOTION_DATABASE_ID` 仅 database 模式需要；
  - 新增环境校验测试，缺 `NOTION_ROOT_PAGE_ID` 时显式失败（`IPC-6001` 语义）。
- Decision level: A（错误处理一致性、可测试性增强、非破坏性实现替换）
- Rationale: 先完成最短可执行闭环，确保前端入口不变但后端可切实跑 page_tree 写入路径。
- Impact:
  - 前端无字段返工，调用命令名和包络保持不变；
  - 生产默认模式切到 page_tree，database 可作为应急回滚；
  - `cargo test -q` 全量通过，当前进入真实变量冒烟阶段。
- Owner: Backend Lead
- Follow-up:
  1. 补充 page_tree mock Notion 集成测试（迁移分类/重复更新/失败重试）。
  2. 在真实环境执行 `cargo run --bin notion_smoke` 并回写 `job_run_id`。

## DL-2026-02-26-30
- Date: 2026-02-26
- Decision: page_tree 同步链路补齐“可回归集成测试 + SQLite 冲突修复”：
  - 引入 `NotionTreeClient` 端口并由 `NotionHttpClient` 适配实现；
  - 新增 `tests/notion_page_tree_service.rs`，覆盖首次创建、重复更新、分类迁移、失败入死信；
  - 修复 `notion_tree_nodes` item upsert 冲突目标为 `ON CONFLICT(id)`，规避 partial unique index 不可匹配问题。
- Decision level: A（稳定性、错误处理一致性、测试补齐）
- Rationale: 在不改 IPC 契约前提下，确保 page_tree 主流程在本地可稳定验证，降低真实 Notion 冒烟前的不确定性。
- Impact:
  - `cargo test -q --test notion_page_tree_service` 通过（4/4）；
  - `cargo test -q` 全量回归通过；
  - 当前唯一阻塞收敛为真实环境变量缺失（`HELPER_DB_KEY/NOTION_TOKEN/NOTION_ROOT_PAGE_ID`）。
- Owner: Backend Lead
- Follow-up:
  1. 在同一终端会话补齐缺失变量后执行 `cargo run --bin notion_smoke`。
  2. 回写 `job_run_id` 与计数结果，完成 P0 冒烟闭环。

## DL-2026-02-26-31
- Date: 2026-02-26
- Decision: `notion_smoke` 标准输出补充 `job_run_id` 字段，输出结构统一为 `{ job_run_id, report }`。
- Decision level: A（可观测性增强，非破坏性变更）
- Rationale: 解决冒烟执行后只能从日志侧检索 job_run_id 的低效问题，提升验收可审计性。
- Impact:
  - 终端可直接拿到 `job_run_id + 关键计数`；
  - 不改业务流程和错误码语义；
  - `cargo test -q` 全量通过。
- Owner: Backend Lead
- Follow-up:
  1. 用真实待同步数据再执行一次冒烟，产出非零 `attempted` 的验收记录。

## DL-2026-02-26-32
- Date: 2026-02-26
- Decision: page_tree 运行参数按“env > settings > default”生效，并补只读 IPC 配置快照：
  - `NOTION_TREE_TIMEZONE > notion.tree.timezone > Asia/Shanghai`；
  - `NOTION_TREE_CATEGORY_GROWTH_LIMIT > notion.tree.category_growth_limit >= 0`（默认 `32`）；
  - `category_growth_limit=0` 时禁止自动扩类，路由到 unknown 并写入 `route_reason=growth_limit_exceeded`；
  - 新增只读 IPC `get_notion_sync_config_snapshot` 暴露模式、配置就绪状态、时区与增长上限。
- Decision level: B（参数策略与非关键接口扩展，已留痕）
- Rationale: 以最小改动提升 page_tree 的长期可运维性，降低前端误判“配置生效状态”的联调成本。
- Impact:
  - 时区不再硬编码 +08:00，非法值显式报错；
  - 分类扩张行为可控且可解释；
  - database/page_tree 双模式回归通过（`cargo test -q` 全绿）。
- Owner: Backend Lead
- Follow-up:
  1. 补 page_tree 专用冒烟回查，形成非零 attempted 的线上验收记录。

## DL-2026-02-26-33
- Date: 2026-02-26
- Decision: 将 `notion_smoke` 收口为双模式可执行并修复 page_tree 路径 runtime 崩溃：
  - 支持 `database/page_tree` 分派并在报告中输出 `report.mode`；
  - page_tree 冒烟改为回查页面树实体（`page_id + meta_block_id + summary_block_id`）；
  - 修复 `futures::executor::block_on` 触发的 `there is no reactor running`，改用 Tokio runtime 执行。
- Decision level: A（稳定性与可观测性修复，非破坏性）
- Rationale: 解除真实冒烟执行时的进程 panic，确保 page_tree 模式可长期稳定验收。
- Impact:
  - `cargo run --bin notion_smoke` 可在 page_tree 模式稳定执行并产出 `job_run_id`；
  - 当无 pending 时返回 `attempted=0` 且状态 `skipped`，不再崩溃；
  - `cargo test -q` 全量回归通过。
- Owner: Backend Lead
- Follow-up:
  1. 生成至少 1 条 pending 数据后，完成一次非零 attempted 的 page_tree 冒烟验收。

## DL-2026-02-26-34
- Date: 2026-02-26
- Decision: 修复桌面端小红书采集入口的 Tokio runtime 缺失问题：
  - `app_core.collect_xhs_with_conn` 从 `futures::executor::block_on` 改为 Tokio current-thread runtime 执行；
  - 避免在 reqwest 异步调用链上触发 `there is no reactor running` panic。
- Decision level: A（稳定性修复，非破坏性）
- Rationale: 与用户上报崩溃栈一致，属于 P0 稳定性问题，需立即闭环。
- Impact:
  - 小红书采集从“可能直接崩溃”恢复为“可返回受控错误/成功结果”；
  - 不改 IPC 契约、不改业务字段；
  - `cargo test -q` 全量通过。
- Owner: Backend Lead
- Follow-up:
  1. 在桌面端执行一次真实 xhs 同步，确认不再出现 runtime panic。

## DL-2026-02-26-35
- Date: 2026-02-26
- Decider: Aiden（Director）+ 文档监工线程
- Decision: Gate A 契约文档收口到发布域 v1，补齐 `get_publish_history` 的筛选参数、legacy 回退语义与错误样例。
- Decision level: A（非破坏性文档校准）
- Rationale: 解决“前后端对历史查询语义理解不一致”导致的联调返工风险。
- Impact scope:
  - `/Users/oliver/Documents/helper/.ops/context/publish-ipc-contract-v1.md`
  - 前端发布中心/系统健康对历史列表的筛选与空结果处理逻辑
  - 后端 `get_publish_history` 的验收口径
- Rollback condition:
  1. 若文档定义与 `backend/src/ipc/commands.rs` 现行实现不一致且无法当天修正；
  2. 若联调验证发现 legacy 路径语义错误导致历史数据误读。
- Rollback action:
  1. 回退到上一版 contract 文档；
  2. 在同日追加修订条目并重新核验示例。
- Owner: Director（文档门禁）

## DL-2026-02-26-36
- Date: 2026-02-26
- Decider: Aiden（Director）+ Product/Architecture
- Decision: 固化“Notion 最终呈现、App 处理引擎”边界，并同步更新 IA/发布域 ADR 映射到现行实现。
- Decision level: B（跨模块边界定义调整，需留痕）
- Rationale: 避免 Notion 与 App 职责混淆，稳定前后端页面职责与验收口径。
- Impact scope:
  - `/Users/oliver/Documents/helper/.ops/context/ADR-IA-v2.md`
  - `/Users/oliver/Documents/helper/.ops/context/ADR-Publish-domain-model.md`
  - `/Users/oliver/Documents/helper/.ops/context/ADR-Notion-as-final-surface.md`
  - 前端 `App.tsx` 四导航职责说明与后端发布域命令使用边界
- Rollback condition:
  1. 若边界定义阻断当前发布闭环（收件箱->审核->发布->健康）执行；
  2. 若边界定义与已上线 IPC 契约发生不可兼容冲突。
- Rollback action:
  1. 恢复上一版 ADR 文本；
  2. 保留新增 ADR 文件但标注 superseded，并补新决策条目。
- Owner: Director + Product/Architecture

## DL-2026-02-26-37
- Date: 2026-02-26
- Decider: Aiden（Director）+ Product + Backend
- Decision: 新增预算与阶段门禁文档基线：
  - `ADR-AI-budget-guard`（月预算 <100，超预算优先降处理量）；
  - `release-gates-v1`（Gate A/B 入场与出场标准）。
- Decision level: B（交付策略与里程碑门禁调整，需留痕）
- Rationale: 把预算策略和发布门禁从“口头约定”升级为“可打勾验收标准”。
- Impact scope:
  - `/Users/oliver/Documents/helper/.ops/context/ADR-AI-budget-guard.md`
  - `/Users/oliver/Documents/helper/.ops/context/release-gates-v1.md`
  - 前后端回报模板与验收流程
- Rollback condition:
  1. 若 Gate 规则导致核心 P0 闭环无法执行且无替代路径；
  2. 若预算守卫策略与线上可执行逻辑冲突并造成持续阻塞。
- Rollback action:
  1. 临时退回上一版门禁清单；
  2. 保留 ADR 但标记为“执行顺序调整”，并在 24 小时内补充修订条目。
- Owner: Director（门禁治理）

## DL-2026-02-26-38
- Date: 2026-02-26
- Decider: Aiden（Director）+ 文档监工 B2
- Decision: B2-G1 路由与目录规则正式入合同/ADR：
  - 固定目录 `Root -> Category -> Week -> Item`；
  - 固定默认分类集合 `学习|工作|生活|健康|财务|灵感|Inbox`；
  - 固定回退语义 `low_confidence` 与 `growth_limit_exceeded`；
  - Root Page 指向检查清单作为上线前必做项。
- Decision level: B（门禁规则与模块边界增强，需留痕）
- Rationale: 先锁定路由与目录语义，避免前后端对 page_tree 行为各自解释。
- Impact scope:
  - `/Users/oliver/Documents/helper/.ops/context/publish-ipc-contract-v1.md`
  - `/Users/oliver/Documents/helper/.ops/context/ADR-Publish-domain-model.md`
  - `/Users/oliver/Documents/helper/.ops/context/ADR-Notion-as-final-surface.md`
  - `/Users/oliver/Documents/helper/.ops/context/notion-root-page-checklist-v1.md`
- Rollback condition:
  1. 若目录/路由规则与 `sync_notion/service.rs` 现行逻辑出现不可兼容冲突；
  2. 若 root page 清单无法支撑实际验收执行。
- Rollback action:
  1. 回退到上一版 ADR/contract；
  2. 同日补充修订决策并重新出具清单。
- Owner: Director（B2 文档门禁）

## DL-2026-02-26-39
- Date: 2026-02-26
- Decider: Aiden（Director）+ 文档监工 B2
- Decision: B2-G2 结构化内容 schema 入合同并锁定兼容窗口：
  - schema 字段固定 `summary/key_points/tags/images/route_reason`；
  - 成功/降级/失败三类样例 JSON 作为验收基线；
  - 旧字段保留周期截止 `2026-03-31`，`2026-04-01` 起未满足 schema 的新增变更禁止合并。
- Decision level: B（合同能力扩展与兼容窗口治理，需留痕）
- Rationale: 在实现前先明确可执行定义，避免 B2 合并后再补合同造成返工。
- Impact scope:
  - `/Users/oliver/Documents/helper/.ops/context/publish-ipc-contract-v1.md`
  - `/Users/oliver/Documents/helper/.ops/context/ADR-Publish-domain-model.md`
  - `/Users/oliver/Documents/helper/.ops/context/ADR-Notion-as-final-surface.md`
- Rollback condition:
  1. 若兼容窗口日期与发布计划冲突并造成无法执行；
  2. 若 schema 定义与现网数据结构无法建立映射关系。
- Rollback action:
  1. 保留 schema 定义，调整窗口日期并追加新决策条目；
  2. 在 Gate B 中标记暂不放行并返回补证据。
- Owner: Director（B2 合同治理）

## DL-2026-02-26-40
- Date: 2026-02-26
- Decider: Aiden（Director）+ 文档监工 B2
- Decision: B2-G3 Gate B 验收包标准化：
  - `release-gates-v1` 增补 B2 必测场景与证据字段；
  - 新增统一填报模板 `b2-acceptance-template-v1.md`；
  - 明确“不接受口头通过”。
- Decision level: B（里程碑验收标准增强，需留痕）
- Rationale: 把 B2 验收从“描述性汇报”改成“结构化证据”，降低漏检风险。
- Impact scope:
  - `/Users/oliver/Documents/helper/.ops/context/release-gates-v1.md`
  - `/Users/oliver/Documents/helper/.ops/context/b2-acceptance-template-v1.md`
  - 前后端验收提交流程
- Rollback condition:
  1. 若模板字段与 Gate B 证据要求不一致；
  2. 若新模板导致验收流程不可执行。
- Rollback action:
  1. 回退模板版本并保留现有 Gate 条目；
  2. 在 24 小时内补发修订模板。
- Owner: Director（B2 验收治理）
