# 后端执行文档：Notion「页面树模式」改造（可直接下发）

## 1. 目标摘要
把当前 Notion「单数据库写入」改成「Personal 根页面下的页面树写入」，并保持现有 IPC 契约不破坏。  
最终结构为：`Personal -> 分类页 -> 周归档页 -> 单条内容页`。

本次已锁定需求（来自用户确认）：
1. 模式：页面树 + 每条内容独立子页面。
2. 根节点：使用 `NOTION_ROOT_PAGE_ID`（Integration 需具备 Add connections 授权）。
3. 分类策略：`预置清单 + 自动扩展`，未知/低置信内容进入 `Inbox`。
4. 去重策略：同一内容只保留一页并更新，不重复建页。
5. 归档粒度：按周归档，页名格式 `YYYY年第WW周`。
6. 迁移分类：自动移动到新分类并记录分类历史。
7. 模板：精简模板。
8. 命名：优先复用现有同名分类页；命名风格优先跟随现有页面；默认分类首批为 4 类 + Inbox。
9. 失败策略：Notion API 重试 3 次后入失败队列。

## 2. 必改接口与配置（决策完成）
### 2.1 环境变量
1. 新增必填：`NOTION_ROOT_PAGE_ID`（真实值由运维注入，不写入仓库）。
2. 新增可选：`NOTION_SYNC_MODE`，默认 `page_tree`，保留 `database` 仅作回滚开关。
3. 保留：`NOTION_TOKEN`、`HELPER_DB_PATH`、`HELPER_DB_KEY`。
4. `NOTION_DATABASE_ID` 改为仅 `database` 模式读取，`page_tree` 模式不依赖。

### 2.2 app_config 默认项（新增）
1. `notion.sync.mode = page_tree`
2. `notion.tree.timezone = Asia/Shanghai`
3. `notion.tree.route_min_confidence = 0.72`
4. `notion.tree.unknown_category = Inbox`
5. `notion.tree.default_categories = 学习|工作|生活|健康|财务|灵感|Inbox`
6. `notion.tree.category_growth_limit = 32`

### 2.3 公开行为约束
1. 不新增 IPC 命令名，不改现有请求/响应 envelope。
2. `run_notion_sync_once` 保持入口不变，只切换内部写入策略。
3. 失败仍返回 `IPC-6001/DB-4001/INT-9000` 体系，不改错误码语义。

## 3. 数据模型变更（SQLite）
新增 migration：`/Users/oliver/Documents/helper/backend/src/storage/migrations/006_notion_page_tree.sql`

新增表 `notion_tree_nodes`：
1. `id TEXT PRIMARY KEY`
2. `node_type TEXT NOT NULL`（`category|week|item`）
3. `node_key TEXT NOT NULL`
4. `page_id TEXT NOT NULL`
5. `parent_page_id TEXT NOT NULL`
6. `title TEXT NOT NULL`
7. `normalized_item_id TEXT`
8. `category_name TEXT`
9. `week_key TEXT`
10. `meta_block_id TEXT`
11. `summary_block_id TEXT`
12. `category_history_json TEXT NOT NULL DEFAULT '[]'`
13. `created_at TEXT NOT NULL`
14. `updated_at TEXT NOT NULL`

索引与约束：
1. `UNIQUE(node_type, parent_page_id, node_key)`
2. `UNIQUE(normalized_item_id) WHERE normalized_item_id IS NOT NULL`
3. `INDEX(node_type, parent_page_id)`

## 4. 模块改造方案（按文件）
### 4.1 新增/调整文件
1. 新增 `/Users/oliver/Documents/helper/backend/src/sync_notion/tree.rs`  
职责：页面树路由、分类决策、周归档键计算、标题清洗、模板渲染。
2. 改造 `/Users/oliver/Documents/helper/backend/src/sync_notion/notion_api.rs`  
新增能力：`list_child_pages`、`create_child_page`、`move_page`、`append_blocks`、`update_paragraph_block`。
3. 改造 `/Users/oliver/Documents/helper/backend/src/sync_notion/service.rs`  
把 `sync_pending_with_conn_with_options` 内部切到 `page_tree` 路径。
4. 改造 `/Users/oliver/Documents/helper/backend/src/ipc/commands.rs`  
`run_notion_sync_once` 读取 `NOTION_SYNC_MODE` 与 `NOTION_ROOT_PAGE_ID`，模式分派。
5. 改造 `/Users/oliver/Documents/helper/backend/src/storage/db.rs`  
注册 `006_notion_page_tree` migration。
6. 改造 `/Users/oliver/Documents/helper/backend/src/storage/migrations/002_seed_defaults.sql`  
补默认配置键。

### 4.2 写入算法（必须严格实现）
1. 加载 pending 记录，得到 `labels + confidence + title + summary + normalized_item_id`。
2. 分类决策：`confidence < 0.72 -> Inbox`；否则先映射默认 4 类；都不命中则自动扩展分类名。
3. 确保分类页存在：优先复用 Personal 下同名页，不存在则创建。
4. 计算周键：按 `Asia/Shanghai` + ISO Week，显示名 `YYYY年第WW周`。
5. 确保周归档页存在：在分类页下按周名复用或创建。
6. upsert 单条内容页：
   - 优先查本地 `notion_tree_nodes(normalized_item_id)`；
   - 有则复用；父级变化则 `move_page` 并更新 `category_history_json`；
   - 无则在周页下创建新页。
7. 页面标题策略：原始标题优先 + 去噪 + 截断（建议 80 字符）。
8. 页面内容模板（精简）：
   - 第一段 metadata（状态、来源、URL、标签、首次/最近出现、分类、normalized_id）；
   - 第二段 summary；
   - 首次创建记录 `meta_block_id` 与 `summary_block_id`，后续仅更新这两个 block。
9. 同步成功后更新 `sync_records`：`target_record_id=page_id`、`sync_state=success`。
10. 失败按现有 retry/backoff/dead_letter 逻辑保持不变。

## 5. 分类映射与命名规则（固定）
1. 首批固定分类：`Journal`、`Travel Planner`、`Habit Tracker`、`Reading List`、`Inbox`。
2. 复用优先：若 Personal 下已有同名页，直接复用，不新建。
3. 命名风格：默认分类优先跟现有页面风格；自动扩展分类使用模型标签原文。
4. 状态流转字段写入 metadata：`Inbox -> Reviewed -> Archived`。

## 6. 测试清单（必须完成）
### 6.1 单元测试
1. 周归档命名测试：跨周边界、跨年边界（Asia/Shanghai）。
2. 分类路由测试：高置信默认分类、低置信入 Inbox、自动扩展分类。
3. 标题清洗与截断测试。
4. 分类迁移历史追加测试。

### 6.2 集成测试（mock Notion client）
1. 首次同步：创建 `分类页+周页+内容页`。
2. 重复同步：不新建内容页，只更新 block。
3. 迁移分类：页面被移动到新分类周页，历史有记录。
4. 同名分类复用：不重复建分类页。
5. 失败重试：3 次后进入 dead_letter，与现有行为一致。
6. 环境变量缺失：`NOTION_ROOT_PAGE_ID` 缺失时返回 `IPC-6001`。

### 6.3 回归
1. `cd /Users/oliver/Documents/helper/backend && cargo test -q`
2. `run_notion_sync_once` 在 `page_tree` 模式下可执行。
3. 旧 `database` 模式 smoke 不回归失败（回滚路径）。

## 7. 交付与验收标准（DoD）
1. `run_notion_sync_once` 在 `page_tree` 模式可写入：
   `Personal -> 分类 -> 周 -> 内容页`。
2. 同一链接重复同步不产生重复内容页。
3. 分类变更会自动移动内容页并记录历史。
4. 低置信内容统一落入 Inbox。
5. 全量测试通过，且 `.ops` 文档更新：
   - `/Users/oliver/Documents/helper/.ops/status/backend-latest.md`
   - `/Users/oliver/Documents/helper/.ops/context/decision-log.md`

## 8. 实施顺序（避免返工）
1. 先做 schema + Notion API 能力层。
2. 再做 service 的 page_tree 主流程。
3. 再做模式分派与配置读取。
4. 最后补测试与回归。

## 9. 假设与默认值（已锁定）
1. Integration 已对 Personal 页面授权。
2. `NOTION_ROOT_PAGE_ID` 由环境变量注入。
3. 页面树模式为默认生产模式；database 模式仅应急回滚。
4. 不引入新付费服务或新外部依赖。
