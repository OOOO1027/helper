# Director Decisions

Date: 2026-02-25
Overall Status: GO (Guarded)

Decision:
- Remove stale automation-era blocker logs that no longer reflect current facts.
- Run tonight using only `前端` / `后端` / `讨论` threads, no new automation threads.

Guardrails:
1. A-level: implementation details, tests, non-breaking refactors can be auto-decided.
2. B-level: priority/order adjustments allowed with decision note in `讨论` thread.
3. C-level: product direction, paid usage, destructive changes must ask user.

Conflict policy:
- If frontend/backend disagree, `讨论` thread does evidence-based arbitration and outputs one decision.

Update (2026-02-25 23:08:43 CST):
- Naming standardization decision (A-level): unify status naming to `GO (Guarded)` across frontend/backend/director docs.
- Scope: status label normalization only; no change to delivery guardrails, priorities, or implementation scope.

Update (2026-02-25 23:42:59 CST):
- Evidence-based sync decision (A-level): backend status note `cargo missing` is stale and must be refreshed.
- Evidence:
  - `cargo -V` => `cargo 1.93.1 (083ac5135 2025-12-15)`
  - `rustc -V` => `rustc 1.93.1 (01f6ddf75 2026-02-11)`
  - `cd /Users/oliver/Documents/helper/backend && cargo test -q` reached test execution and failed at `tests/dead_letter_replay.rs:105` (`retry_dead_letter_replays_wechat_import_parse_path`, left 0 right 1).
- Action:
  1. Backend refreshes `backend-latest.md` runtime facts (toolchain available, one failing test).
  2. Backend either fixes the failing test path or records a risk waiver with root cause.
- Owner: Backend Lead
- Due: 2026-02-26 12:00 CST

Update (2026-02-26 00:10:58 CST):
- Coordination maturity decision (B-level): tracks are collaborative on core flow, but integration closure is lagging on sync/tooling IPCs.
- Evidence:
  - Frontend invokes `get_sync_logs` in `desktop/src/services/ipc.ts`, but no frontend calls for `import_wechat_and_persist` / `run_notion_sync_once` / `retry_dead_letters` / `get_local_tool_status`.
  - Tauri desktop handler currently registers up to `get_sync_logs` only (`desktop/src-tauri/src/main.rs`).
  - Backend has implemented and tested the above IPCs in `backend/src/ipc/commands.rs` and contract examples.
- Determination:
  - Not "各做各的" for the core path (`import_wechat_files`, review queue, sync logs are aligned).
  - Partial divergence exists: backend capability lead over frontend integration surface.
- Action:
  1. Frontend integrates `get_local_tool_status` at app startup + import precheck.
  2. Frontend/desktop expose one-click actions for `run_notion_sync_once` and `retry_dead_letters`.
  3. Backend provides minimal request/response fixtures for the three IPCs in contract examples used by frontend.
- Owner: Frontend Lead + Backend Lead
- Due: 2026-02-26 18:00 CST

Update (2026-02-26 00:13:25 CST):
- Optimal integration plan decision (B-level): execute in two-step order, backend exposure first, frontend wiring second.
- Why this order:
  - Removes invocation uncertainty in one pass (desktop tauri handler + contract fixtures).
  - Prevents frontend rework caused by command name/payload drift.
- Execution order:
  1. Backend completes handler exposure + fixtures for `get_local_tool_status` / `run_notion_sync_once` / `retry_dead_letters` / `import_wechat_and_persist`.
  2. Frontend completes ipc.ts wrappers + UI entry wiring + guarded error states.
- Acceptance:
  - `pnpm tauri dev` can trigger all 4 IPCs from UI paths.
  - success/error envelopes are visible and mapped to user-facing messages.
- Owner: Backend Lead -> Frontend Lead
- Due: Backend by 2026-02-26 14:00 CST; Frontend by 2026-02-26 18:00 CST.

Update (2026-02-26 00:21:11 CST):
- Frontend thread health check decision (B-level): skill execution is active but skewed toward visual polish; integration closure is still incomplete.
- Evidence:
  - Skill-aligned visual implementation exists (`desktop/src/styles.css` gradients/typography/glass/motion tokens).
  - New IPC wrappers exist in `desktop/src/services/ipc.ts`: `importWechatAndPersist`, `retryDeadLetters`, `runNotionSyncOnce`, `getLocalToolStatus`.
  - But these wrappers are not consumed by pages yet (no call-sites outside `ipc.ts`).
  - `ImportCenterPage` still calls `importWechatFiles`; `SettingsLogsPage` currently only reads `getSyncLogs`.
- Determination:
  - Not blocked, but "half-closed" integration state; frontend should stop adding new visual enhancements until IPC usage is wired in UI paths.
- Action:
  1. Import flow switches to `importWechatAndPersist` with persisted-result summary rendering.
  2. Settings page adds manual `runNotionSyncOnce` trigger + `retryDeadLetters` entry.
  3. App startup/import precheck integrates `getLocalToolStatus` and shows guardrail hints.
- Owner: Frontend Lead
- Due: 2026-02-26 18:00 CST

Update (2026-02-26 00:39:23 CST):
- Frontend checkpoint decision (B-level): integration closure task is completed; move frontend priority to reliability UX and sync observability presentation.
- Evidence:
  - `ImportCenterPage` now uses `getLocalToolStatus` precheck + `importWechatAndPersist`.
  - `SettingsLogsPage` now exposes `runNotionSyncOnce` and `retryDeadLetters` actions.
  - `App.tsx` startup path now checks local tool availability and shows guardrail notice.
  - Remaining gap: `getSyncDailyStats` wrapper exists but is not yet surfaced in UI.
- Action:
  1. Frontend adds daily sync stats section in Settings page (success rate, avg success/fail counts).
  2. Frontend updates sync log filters (job type/state/time range) to accelerate troubleshooting.
  3. Frontend adds operation-level loading/disable guards and empty-state messaging for sync actions.
- Owner: Frontend Lead
- Due: 2026-02-26 20:00 CST

Update (2026-02-26 00:46:53 CST):
- Backend progress checkpoint decision (B-level): backend has reached pre-production verification stage; priority shifts from feature expansion to real-env validation and contract closure.
- Evidence:
  - `backend-latest.md` shows runnable path closed through `wechat import -> ... -> notion sync`.
  - `cd /Users/oliver/Documents/helper/backend && cargo test -q` passed in current run.
  - `backend-latest.md` next-step item "frontend startup/import guard" is now stale because frontend has already integrated it.
  - Remaining open issues still include DIR-004 (contract details completion).
- Action:
  1. Backend executes real-key smoke validation: `cargo run --bin notion_smoke` with `HELPER_DB_PATH/HELPER_DB_KEY/NOTION_TOKEN/NOTION_DATABASE_ID`.
  2. Backend writes smoke result summary (success/failure sample, mismatch count, job_run_id) into status update.
  3. Backend + Product/Architecture complete master-spec contract details (contract path, version policy, error-code catalog) and append decision-log entry.
- Owner: Backend Lead (+ Product/Architecture for item 3)
- Due: Smoke by 2026-02-26 18:00 CST; contract closure by 2026-02-26 22:00 CST.

Update (2026-02-26 00:50:06 CST):
- Frontend/backend coordination check decision (B-level): integration cooperation is high; remaining issue is status-sync freshness rather than technical mismatch.
- Evidence:
  - Shared IPCs are end-to-end aligned and wired: `import_wechat_and_persist`, `retry_dead_letters`, `run_notion_sync_once`, `get_local_tool_status`, `get_sync_daily_stats` (backend commands + tauri registration + frontend invocation).
  - Frontend Settings page now consumes `getSyncDailyStats` and refreshes overview after sync/replay actions.
  - Backend status run time is 00:45, while frontend status run time is 00:20; progress sync cadence is not yet tightly aligned.
- Determination:
  - Not "各做各的"; current execution is collaborative with a mostly closed integration loop.
  - Keep DIR-002 open until same-round status freshness is achieved.
- Action:
  1. Frontend refreshes status once after latest settings/statistics integration checkpoint.
  2. Backend/Frontend keep same-round timestamped status updates in `讨论` thread.
- Owner: Frontend Lead + Backend Lead
- Due: 2026-02-26 01:30 CST

Update (2026-02-26 00:53:00 CST):
- Smoke execution blocker decision (A/B-level): resolve shell compatibility first, then unblock env-based smoke run.
- Evidence:
  - Precheck command using `${!v}` failed in zsh (`bad substitution`) because indirect expansion is bash-specific.
  - Re-run via bash confirmed all required variables missing: `HELPER_DB_PATH/HELPER_DB_KEY/NOTION_TOKEN/NOTION_DATABASE_ID`.
- Determination:
  - A-level: replace precheck with shell-neutral command (`printenv`) to avoid shell mismatch.
  - B-level: keep smoke run blocked until user provides required env vars; no workaround by hardcoding secrets.
- Action:
  1. Backend uses shell-neutral precheck command and reports only missing key names.
  2. User provides required env vars in runtime environment (no secret values in repo/logs).
  3. Backend reruns `cargo test -q` and `cargo run --bin notion_smoke`, then reports `job_run_id` and summary.
- Owner: Backend Lead + User
- Due: 2026-02-26 18:00 CST

Update (2026-02-26 15:34:21 CST):
- Smoke result interpretation decision (A-level): runtime/env blocker is cleared, but smoke acceptance remains incomplete due to zero pending records.
- Evidence:
  - `cargo run --bin notion_smoke` executed successfully.
  - Output summary: `scanned=0`, `attempted=0`, `succeeded=0`, `failed=0`, `mismatched=0`.
  - Runtime warning: `notion smoke skipped: no pending sync records`.
- Determination:
  - This is not a crash and not a credential failure.
  - Pre-production acceptance is still NOT passed because real sync path was not exercised.
- Action:
  1. Create at least one pending sync record via import/pipeline path.
  2. Rerun `cargo run --bin notion_smoke` and require `attempted > 0`.
  3. Record `job_run_id` + summary counts in backend status.
- Owner: Backend Lead
- Due: 2026-02-26 18:30 CST

Update (2026-02-26 17:11:55 CST):
- PM health-check decision (B-level): engineering quality baseline is healthy, but release-readiness is still gated by real-sync evidence and governance freshness.
- Evidence:
  - Quality checks passed in this run:
    - `cd /Users/oliver/Documents/helper/backend && cargo test -q` => all passed.
    - `cd /Users/oliver/Documents/helper/desktop && pnpm build` => build passed.
  - Progress docs are not same-round fresh:
    - Frontend status run time: 2026-02-26 16:26:00 CST.
    - Backend status run time: 2026-02-26 02:55:09 CST.
  - Governance gap remains:
    - `master-spec.md` still marks contract source-of-truth as TBD.
    - DIR-004 remains OPEN.
- Determination:
  - Technical execution quality: GREEN (build/test baseline healthy).
  - Delivery readiness: AMBER (missing non-zero real sync smoke evidence + contract governance closure).
- Action:
  1. Backend executes one real pending-sync run and records non-zero smoke evidence (`attempted > 0`, `job_run_id`) in backend status.
  2. Frontend/Backend refresh status docs in same round to close DIR-002 freshness drift.
  3. Product/Architecture + Backend close DIR-004 by updating master-spec contract path/version/error-code catalog and append decision-log entry.
- Owner: Backend Lead + Frontend Lead + Product/Architecture
- Due: Evidence/sync by 2026-02-26 19:00 CST; DIR-004 closure by 2026-02-26 22:00 CST


Update (2026-02-26 18:47:40 CST):
- PM全量巡检裁决（B级，留痕）：项目维持 GO (Guarded)，质量基线通过，但发布前验收仍为 Amber。
- Evidence:
  - 后端质量门：`cd /Users/oliver/Documents/helper/backend && cargo test -q` 全量通过。
  - 前端质量门：`cd /Users/oliver/Documents/helper/desktop && pnpm build` 通过（Vite build completed）。
  - 进度新鲜度存在漂移：frontend 运行时间 `2026-02-26 16:26:00 CST`，backend 运行时间 `2026-02-26 04:41:18 CST`。
  - `director-open-issues.md` 仍有 DIR-002（进度同步纪律）与 DIR-004（契约细节完备）未关闭。
  - 后端状态文件仍未给出“真实 pending 数据下 attempted>0 的 notion_smoke 成功证据”。
- Determination:
  - 工程质量：Green（构建/测试稳定）。
  - 联调与上线就绪：Amber（证据链与治理闭环未完成）。
- Action:
  1. 后端：补一轮真实 pending 同步与 smoke 证据（含 `job_run_id`、`attempted/succeeded/failed`），并刷新 `backend-latest.md`。
  2. 前端：在后端证据更新后执行 P0/P1 手工闭环复验并刷新 `frontend-latest.md`。
  3. Director：待两端同轮时间戳对齐后评估关闭 DIR-002；契约细节入 spec 后评估关闭 DIR-004。
- Owner: Backend Lead + Frontend Lead + Product/Architecture
- Due: 后端证据 2026-02-26 20:00 CST；前端复验 2026-02-26 20:30 CST；治理项复核 2026-02-26 22:00 CST

Update (2026-02-26 19:02:00 CST):
- XHS 空响应解析故障裁决（A-level）：将“EOF 解码失败”改为可操作错误，并增强诊断预览。
- Evidence:
  - 报错点位于 `backend/src/collectors/xhs.rs` 的 `fetch_with_chrome`，`run_osascript` 成功返回但 `stdout` 为空时，`serde_json::from_str` 触发 EOF。
- Determination:
  - 根因是浏览器注入返回空载荷（常见于 Apple Events JS 权限未开启或浏览器返回 undefined/null），不应继续走 JSON decode。
- Action:
  1. 后端已增加空载荷前置校验：`empty/undefined/null` 直接返回可操作 Validation 提示（开启 Chrome 选项或配置 `XHS_COOKIE/XHS_COOKIE_FILE`）。
  2. 后端已增强 decode 失败信息，附 `output_preview` 便于定位。
  3. 回归：`cd /Users/oliver/Documents/helper/backend && cargo test -q` 全量通过。
- Owner: Backend Lead
- Due: Completed

Update (2026-02-26 19:16:00 CST):
- XHS 同步触发桌面端崩溃裁决（A-level）：根因为 cookie 模式使用 `reqwest::blocking` 导致 Tokio 上下文 panic，已改为异步客户端。
- Evidence:
  - macOS crash report `helper-desktop-2026-02-26-191033.ips` 栈帧显示：
    - `tokio::runtime::blocking::shutdown::Receiver::wait`
    - `reqwest::blocking::client::ClientBuilder::build`
    - `helper_backend::collectors::xhs::XhsCollector::fetch_with_cookie`
  - 崩溃信号：`EXC_CRASH (SIGABRT)` / `abort() called`。
- Determination:
  - 该问题属于实现级 A 类故障，不涉及契约变更；可直接热修。
- Action:
  1. 后端已将 `fetch_with_cookie` 切换为 `reqwest::Client` + async `.send().await/.text().await`。
  2. `fetch_page_json` cookie 分支已改为 await。
  3. 回归：`cd /Users/oliver/Documents/helper/backend && cargo test -q` 全量通过。
- Owner: Backend Lead
- Due: Completed

Update (2026-02-26 19:24:00 CST):
- 错误可观察性裁决（A-level）：前端不再把所有 `validation failed` 折叠成同一提示，改为优先显示具体原因。
- Evidence:
  - 用户端仅看到“输入参数不符合要求”，无法区分真实故障（如 xhs 空返回）。
- Determination:
  - 属于实现细节 A 级，可自动裁决并立即修复。
- Action:
  1. 前端 `desktop/src/services/ipc.ts` 增加 `xhs browser fetch returned empty payload` 专项提示。
  2. 对其他 validation 错误，展示去前缀后的具体 detail。
  3. 验证：`cd /Users/oliver/Documents/helper/desktop && pnpm build` 通过。
- Owner: Frontend Lead
- Due: Completed

Update (2026-02-26 19:31:00 CST):
- XHS 500 回退策略裁决（A-level）：当 `XHS_COOKIE` 直连返回 500/401/403 或 JSON 解码失败时，自动回退到浏览器注入路径。
- Evidence:
  - 用户实测提示：`xhs request rejected: http_status=500 Internal Server Error`。
- Determination:
  - 属于实现级容错增强，不改 IPC 契约，可自动裁决。
- Action:
  1. `backend/src/collectors/xhs.rs` 在 `fetch_page_json` 中加入 cookie 失败 -> chrome fallback。
  2. 新增 `should_fallback_to_chrome` 判定函数。
  3. 验证：`cd /Users/oliver/Documents/helper/backend && cargo test -q` 全量通过。
- Owner: Backend Lead
- Due: Completed

Update (2026-02-26 19:39:00 CST):
- XHS 浏览器返回非 JSON 容错裁决（A-level）：将 `xhs browser body decode failed` 从 internal 转为可操作 validation，并增加状态码/HTML/空体识别。
- Evidence:
  - 用户反馈：`internal failed: xhs browser body decode failed: expected value at line 1 column 1`。
- Determination:
  - 根因是浏览器抓取返回的 body 非 JSON（常见登录页/风控页 HTML），应提供可执行提示而非 internal。
- Action:
  1. `fetch_with_chrome` 新增 non-2xx 检查并返回 `http_status + body_preview`。
  2. 新增空体与 HTML 检测（提示登录/风控）。
  3. JSON 解析失败改为 validation 并附 preview。
  4. 回归：`cd /Users/oliver/Documents/helper/backend && cargo test -q` 全量通过。
- Owner: Backend Lead
- Due: Completed

Update (2026-02-26 19:46:00 CST):
- XHS 网关 500 根因修复裁决（A-level）：将 Chrome 采集从同步 XHR 切回异步 fetch（轮询结果），以兼容页面请求链路并避免 `jarvis-gateway-default` 500。
- Evidence:
  - 用户报错：`xhs browser request rejected: http_status=500, body_preview=create invoker failed, service: jarvis-gateway-default`。
  - 同步 XHR 可能绕过页面侧 fetch 链路，导致请求不符合网关预期。
- Determination:
  - 实现级 A 类故障，可自动裁决并热修，不改 IPC 契约。
- Action:
  1. `fetch_with_chrome` 改为：注入 async fetch -> 写入 window 临时键 -> AppleScript 轮询取结果 -> 清理键。
  2. 保留空载荷、非 2xx、HTML 响应、非 JSON 响应的可操作报错。
  3. 验证：`cd /Users/oliver/Documents/helper/backend && cargo test -q` 全量通过。
- Owner: Backend Lead
- Due: Completed

Update (2026-02-26 20:02:00 CST):
- XHS 同步根修裁决（A-level，大改）：新增“个人主页 Tab 抓取”降级主备链路，绕过 `jarvis-gateway-default` 500。
- Evidence:
  - 多轮重试后仍稳定复现：`xhs browser request rejected: http_status=500, body_preview=create invoker failed, service: jarvis-gateway-default`。
  - 该错误来自 Web API 通道（cookie/chrome fetch）在网关层拒绝，非前端参数问题。
- Determination:
  - 对现有 API 通道保留，但在满足失败特征时自动切换到页面 DOM 抓取（点赞/收藏 Tab）。
- Action:
  1. `backend/src/collectors/xhs.rs` 重构 `collect_internal`：API 失败命中特征时自动 fallback 到 profile tabs scrape。
  2. 新增 profile 抓取实现：自动进入“我”主页、切换“点赞/收藏”Tab、滚动分页、抽取 `explore` 链接并生成 `CollectedItem`。
  3. 保留并增强原 API 错误诊断；新增 fallback 判定规则（500/invoker/json decode）。
  4. 回归：`cd /Users/oliver/Documents/helper/backend && cargo test -q` 全量通过。
- Owner: Backend Lead
- Due: Completed

Update (2026-02-26 20:12:00 CST):
- XHS SOP 重写裁决（A-level）：修复“首页盲滚、应用无反馈”问题，改为强制个人页 + 强制 tab + 有界抓取。
- Evidence:
  - 用户反馈：点击“一键同步小红书”后浏览器仅在首页滚动，APP 端无可见进展。
  - 原降级脚本未严格校验是否进入 `/user/profile/` 且是否激活目标 tab。
- Determination:
  - 属于实现细节 A 级故障；直接重写 profile scrape SOP。
- Action:
  1. 新增强制导航校验：优先定位 `a[href*="/user/profile/"]` 并进入个人页，未进入则报 `profile_navigation_failed`。
  2. 新增 tab 锁定：目标 tab（点赞/收藏）未激活会重复点击校正。
  3. 新增抓取有界策略：`XHS_PROFILE_ROUNDS` 默认 12（4..30），`XHS_PROFILE_MAX_ITEMS` 默认 120（20..500），防止长时间盲滚。
  4. 保留 API 路径，命中 500/invoker/json 失败自动降级到 profile 抓取。
  5. 回归：`cd /Users/oliver/Documents/helper/backend && cargo test -q` 全量通过。
- Owner: Backend Lead
- Due: Completed

Update (2026-02-26 20:18:00 CST):
- 审核队列状态映射修复裁决（A-level）：前端 `exception` 标签错误传参 `failed`，已改为后端契约值 `rejected`。
- Evidence:
  - 用户报错：`state must be one of pending|processing|done|rejected, got failed`。
- Determination:
  - 属于前端实现细节错误（A 级），可自动裁决并立即修复。
- Action:
  1. `desktop/src/pages/ReviewQueuePage.tsx`：`tabToFilter(exception)` 从 `failed` 改为 `rejected`。
  2. 同文件补齐状态展示：`rejected` 归为异常，`processing` 归为待审核。
  3. `desktop/src/services/adapters.ts`：`rejected` 归类到异常 tab，`processing` 归类到 pending。
  4. 验证：`cd /Users/oliver/Documents/helper/desktop && pnpm build` 通过。
- Owner: Frontend Lead
- Due: Completed

Update (2026-02-26 20:26:00 CST):
- 同步配置与XHS入库双问题收口裁决（B-level，留痕）：修复配置命令不兼容与XHS去重提前中断导致“抓到多、入库0”的核心缺陷。
- Evidence:
  - 用户报错：`同步配置加载失败`；前端实际调用 `get_sync_config_snapshot`，后端注册的是 `get_notion_sync_config_snapshot`。
  - 用户现象：XHS 显示抓取 138 条，但 APP/Notion/审核队列无新增。
  - 后端实现中 `collect_xhs_with_conn` 在首个已存在 fingerprint 处直接 `break`，会在混合顺序数据源下提前中断。
- Determination:
  - 配置加载失败属跨端命令名不一致；XHS“抓到多不入库”属后端处理策略缺陷；均为 A/B 级可直接修复项。
- Action:
  1. 前端 `desktop/src/services/ipc.ts`：
     - `getSyncConfigSnapshot` 优先调用 `get_notion_sync_config_snapshot`；
     - 兼容旧命令 `get_sync_config_snapshot`；
     - 双命令都不存在时回退 fallback。
  2. 后端 `backend/src/app_core/mod.rs`：
     - XHS 指纹命中从 `break` 改为 `continue`，记录 `skipped_existing`，避免提前终止。
  3. 测试更新 `backend/tests/app_core_real_db_paths.rs`：
     - 用例改为“跳过已存在并继续”，断言 `stored=2`、`xhs_count=3`。
  4. 前端文案增强 `desktop/src/services/ipc.ts`：补充 invalid timezone / growth limit 错误映射。
  5. 验证：
     - `cd /Users/oliver/Documents/helper/backend && cargo test -q` 通过；
     - `cd /Users/oliver/Documents/helper/desktop && pnpm build` 通过。
- Owner: Director (implemented) + Backend Lead + Frontend Lead
- Due: Completed

Update (2026-02-26 20:34:00 CST):
- 数据可见性收口裁决（B-level，留痕）：修复“XHS抓取有数量但审核队列无记录”的两处核心路径。
- Evidence:
  - 用户反馈：审核队列无记录；同步配置加载失败；XHS显示抓取138条但APP/Notion无可见数据。
  - 代码核对：
    - 前端调用 `get_sync_config_snapshot`，而后端注册命令为 `get_notion_sync_config_snapshot`。
    - 后端 `collect_xhs_with_conn` 遇首个已存在 fingerprint 会 `break` 提前中断。
    - pipeline 当前固定评分会使多数内容不触发审核（`review_required=false`），导致审核队列可为空。
- Determination:
  - 需同时修复：命令兼容、XHS入库中断、XHS审核可见性。
- Action:
  1. 前端 `desktop/src/services/ipc.ts`：同步配置命令兼容双命令名。
  2. 后端 `backend/src/app_core/mod.rs`：XHS去重命中改 `continue`，不再提前终止。
  3. 后端 `backend/src/app_core/mod.rs`：XHS来源默认 `force_review=true`，确保新抓取项进入审核队列可见。
  4. 测试与构建：
     - `cd /Users/oliver/Documents/helper/backend && cargo test -q` 通过；
     - `cd /Users/oliver/Documents/helper/desktop && pnpm build` 通过。
- Owner: Director (implemented) + Backend Lead + Frontend Lead
- Due: Completed

Update (2026-02-26 20:41:00 CST):
- 审核队列仍为空的追加修复（A-level）：为历史重复项增加 XHS 审核回填，确保“抓取成功但全重复”场景也能在审核队列看到记录。
- Evidence:
  - 用户反馈：修复后审核队列仍无任何记录。
  - 原逻辑对已存在 fingerprint 仅 `continue` 跳过，不会把历史记录回填到 review_queue。
- Determination:
  - 实现细节 A 级，需补充回填机制。
- Action:
  1. `backend/src/app_core/mod.rs`：新增 `ensure_xhs_review_queue_for_fingerprint`，在命中重复时尝试为对应历史分析结果回填 `review_queue(state=pending)`。
  2. `collect_xhs_with_conn` 增加 `backfilled_existing` 统计并写入日志。
  3. 回归：`cd /Users/oliver/Documents/helper/backend && cargo test -q` 通过。
- Owner: Backend Lead
- Due: Completed

Update (2026-02-26 20:44:00 CST):
- XHS 审核可见性兜底裁决（A-level）：即使预算保护关闭审核，XHS 来源仍强制入审核队列。
- Evidence:
  - 用户持续反馈审核队列无记录；预算开关可能导致 `review_enabled=false` 时跳过入队。
- Determination:
  - XHS 是本轮主线来源，需对预算策略做来源级例外，保证可见性与可操作性。
- Action:
  1. `backend/src/app_core/mod.rs` 将入队条件改为 `review_required && (budget.review_enabled() || force_review)`。
  2. 回归：`cd /Users/oliver/Documents/helper/backend && cargo test -q` 全量通过。
- Owner: Backend Lead
- Due: Completed

Update (2026-02-26 20:58:00 CST):
- 审核队列可读性与状态流转修复裁决（B-level，留痕）：解决“标题看不清、右侧详情难读、异常/已完成长期为空”的体验阻塞。
- Evidence:
  - 前端 `toReviewItem` 在缺标题时降级为 `条目 + 短ID`，导致列表可读性差。
  - 后端 `ReviewItem` 查询未返回 `title/url`，前端缺少可读字段来源。
  - 前端审核动作将“通过/驳回/重复”统一写入 `done`，异常页缺少真实承接数据。
- Determination:
  - 属于跨端衔接与状态语义偏差，按 B 级执行：保留 IPC 契约，补齐字段和状态映射，不做破坏性改造。
- Action:
  1. 后端 `backend/src/app_core/mod.rs`、`backend/src/app_core/adapters/sqlite_core_port.rs`：`ReviewItem` 补 `title/url`，审核查询改联表回填可读标题与来源链接。
  2. 前端 `desktop/src/pages/ReviewQueuePage.tsx`：审核决策改为 `approved->done`、`rejected/duplicate->rejected`；标签增加三态数量徽标；空状态文案解释“为什么为空”。
  3. 前端 `desktop/src/services/adapters.ts`：移除“条目+ID”弱可读回退，改为来源型回退标题。
  4. 前端 `desktop/src/components/ReviewDrawer.tsx`、`desktop/src/styles.css`：详情区补状态字段并提升对比度、长ID可读性、布局边界。
  5. 回归：
     - `cd /Users/oliver/Documents/helper/backend && cargo test -q` 通过；
     - `cd /Users/oliver/Documents/helper/desktop && pnpm build` 通过。
- Owner: Director (implemented) + Frontend Lead + Backend Lead
- Due: Completed

Update (2026-02-26 21:14:00 CST):
- 审核页认知负担收敛裁决（A-level）：解决“227 vs 32 口径误解、技术字段干扰、触发原因难懂”。
- Evidence:
  - 用户反馈：将“今日采集”误解为“待审核总盘子”，并对 `ID/归一化ID/触发原因` 表达不可理解。
  - 现状：状态条缺口径说明；审核抽屉默认暴露技术字段；原因直接显示技术字符串。
- Determination:
  - 属于实现细节与交互文案问题（A 级），可自动裁决并立即修复。
- Action:
  1. `desktop/src/components/StatusStrip.tsx`：补充口径副文案（当日新增、队列累计、本月成本占比）。
  2. `desktop/src/pages/DashboardPage.tsx`：首页文案明确“今日采集 vs 待审核”不是同一分母。
  3. `desktop/src/services/adapters.ts`：审核 `reason` 人话化映射（XHS/模型触发/manual 处理）。
  4. `desktop/src/components/ReviewDrawer.tsx`：ID 与归一化ID移至“高级信息（排查用）”折叠区，默认不展示。
  5. `desktop/src/styles.css`：新增状态条副文案与高级信息折叠样式。
  6. 验证：`cd /Users/oliver/Documents/helper/desktop && pnpm build` 通过。
- Owner: Director (implemented) + Frontend Lead
- Due: Completed
- Follow-up (2026-02-26 21:16:00 CST): `StatusStrip` 的预算占用来源统一为 `getDashboardMetrics().budget_usage_ratio`，避免顶部与仪表盘口径不一致。

Update (2026-02-26 21:38:00 CST):
- XHS 审核策略切换（C-level，用户已确认 A 方案）：从“XHS 默认人工审核”切换为“XHS 正常项自动通过，仅异常项进人工审核”。
- User confirmation:
  - 用户在本线程明确选择 A 方案（推荐项）。
- Evidence:
  - 用户反馈“XHS 应该自动通过”；并且“已完成为空”影响可感知进度。
  - 旧策略 `force_review=true` 导致 XHS 条目默认进入 `pending`，done 侧不可见。
- Determination:
  - 按 C 级决策执行策略变更，并补历史策略遗留数据迁移，避免切换后口径断层。
- Action:
  1. `backend/src/app_core/mod.rs`
     - 新增 XHS 异常判定（低质量/低置信度/极短内容/评分规则触发）;
     - `preview_pipeline/execute_pipeline_with_conn` 对齐新策略：XHS 正常项不进待审核；
     - XHS 正常项自动写入 `review_queue(state=done, reason=XHS auto-pass normal item)`；
     - 保留异常项进入 `pending(reason=XHS anomaly requires manual review)`；
     - 新增 `apply_xhs_auto_pass_policy`：历史 XHS `pending` 自动迁移到 `done`（异常项保留 pending）。
  2. `backend/src/app_core/mod.rs`（backfill 路径）
     - `ensure_xhs_review_queue_for_fingerprint` 改为按异常判定写 `pending/done`，不再一律 pending。
  3. `desktop/src/services/adapters.ts`
     - 新增 XHS 新原因文案映射（自动通过/异常待审）。
  4. 测试：
     - `backend/tests/app_core_real_db_paths.rs` 新增策略迁移回归测试；
     - 原 xhs collect 测试补 done 断言。
  5. 验证：
     - `cd /Users/oliver/Documents/helper/backend && cargo test -q` 通过；
     - `cd /Users/oliver/Documents/helper/desktop && pnpm build` 通过。
- Owner: Director (implemented) + Backend Lead + Frontend Lead
- Due: Completed

Update (2026-02-26 21:47:00 CST):
- 审核后可观测性补强裁决（A-level）：审核动作成功后自动触发一次 Notion 同步并在审核页直接回显结果。
- Evidence:
  - 用户反馈：审核后无法直观看到是否已进入 Notion，流程不丝滑。
- Determination:
  - 属于交互链路可见性不足（A 级），无需改 IPC 契约，前端可直接收口。
- Action:
  1. `desktop/src/pages/ReviewQueuePage.tsx`：`applyDecision` 成功后自动调用 `runNotionSyncOnce(20)`。
  2. 同步结果（成功/失败/尝试数）写入当前反馈、通知与会话日志。
  3. 同步失败时给出可操作下一步（去设置与日志手动同步/查看失败日志）。
  4. 页面说明文案改为“审核后自动触发一次 Notion 同步并回显结果”。
  5. 验证：`cd /Users/oliver/Documents/helper/desktop && pnpm build` 通过。
- Owner: Director (implemented) + Frontend Lead
- Due: Completed

Update (2026-02-26 22:03:00 CST):
- “227 可解释性 + Notion可见性”收口裁决（B-level，留痕）：把“采集总数”变成可追踪去向，不再只显示裸数字。
- Evidence:
  - 用户连续反馈：看不懂 227 代表什么、去向不透明；审核后无法判断是否进入 Notion。
- Determination:
  - 不改现有主流程与契约语义，新增只读观测接口 + 前端总览面板，优先解释力与可见性。
- Action:
  1. 后端新增 `CollectionInsight`（今日总量/来源分布/审核去向/Notion同步状态/最近采集样本）。
     - 文件：`backend/src/app_core/mod.rs`
     - IPC：`get_collection_insight`（`backend/src/ipc/commands.rs`）
     - Tauri 注册：`desktop/src-tauri/src/main.rs`
  2. 前端新增仪表盘“今日采集去向总览”面板：
     - 展示公式：`today_total = 待审 + 已完成 + 异常 + 直过`；
     - 展示 Notion `success/pending/retry/failed/not_started`；
     - 展示最近 12 条样本（来源/标题/审核状态/Notion状态/采集时间）。
     - 文件：`desktop/src/pages/DashboardPage.tsx`、`desktop/src/services/ipc.ts`、`desktop/src/types/contracts.ts`、`desktop/src/styles.css`
  3. 审核后 Notion 反馈已在上一轮补齐：审核成功后自动触发一次 `run_notion_sync_once(20)` 并回显结果。
- Validation:
  - `cd /Users/oliver/Documents/helper/backend && cargo test -q` 通过；
  - `cd /Users/oliver/Documents/helper/desktop && pnpm build` 通过；
  - `cd /Users/oliver/Documents/helper/desktop/src-tauri && cargo check -q` 通过。
- Owner: Director (implemented) + Frontend Lead + Backend Lead
- Due: Completed
