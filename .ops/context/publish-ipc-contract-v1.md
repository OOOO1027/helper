# Publish IPC Contract v1

Status: DRAFT
Owner: Backend + Frontend
Last updated: 2026-02-26

## Scope
- 定义发布域相关 IPC 契约，作为前后端联调和回归测试基线。
- 本文覆盖：
  - `ingest_xhs_incremental`
  - `get_ingest_queue`
  - `get_review_queue`
  - `publish_approved_to_notion`
  - `get_publish_queue`
  - `get_publish_history`
  - `mark_source_inactive`

## Envelope Baseline
- 所有 IPC 返回统一包络：
  - `ok: boolean`
  - `request_id: string`
  - `data: T | null`
  - `error: { code, message, retryable } | null`

## Commands

### `ingest_xhs_incremental()`
- Request: `{}`
- Response:
  - `fetched: number`
  - `stored: number`
  - `duplicates: number`
  - `failed: number`
- 错误：
  - `IPC-6001` 参数或运行态校验失败
  - `DB-4001` 数据库失败
  - `INT-9000` 内部异常

### `get_ingest_queue({ page })`
- Request:
  - `page.page: number (>0)`
  - `page.page_size: number (>0)`
- Response:
  - `items[]`:
    - `source_item_id`
    - `source`
    - `title`
    - `collected_at`
    - `review_state`
    - `publish_state`
  - `page/page_size/total`

### `get_review_queue({ filter, page })`
- Request:
  - `filter.state?: pending | rejected | done`
  - `filter.min_priority?: number`
  - `page`
- Response:
  - `Paged<ReviewItem>`
  - `ReviewItem` 扩展字段：
    - `quality_band`
    - `publish_state`

### `publish_approved_to_notion({ limit? })`
- Request:
  - `limit?: number`（1-200，服务端会 clamp）
- Response:
  - `request_id`
  - `attempted`
  - `succeeded`
  - `failed`
  - `requeued`
  - `dead_lettered`
- 行为约束：
  - 无候选时返回 0 统计并写 `publish_audit`（`item_id=__batch__`, `status=skipped`）
  - 批次失败时，候选 `publish_tasks` 需转 `failed` 并记录 `error_code`
  - `publish_audit` 同步写入观测字段：`pipeline_stage`、`sync_mode`、`retryable`

### `get_publish_queue({ page })`
- Request:
  - `page`
- Response:
  - `items[]`:
    - `id`
    - `item_id`
    - `title`
    - `state` (`pending|processing|published|failed|ignored`)
    - `attempt_count`
    - `next_retry_at`
    - `error_code`
    - `updated_at`
  - `page/page_size/total`

### `get_publish_history({ range, page, state?, sync_mode?, retryable? })`
- Request:
  - `range.from` / `range.to`（非空）
  - `page.page` / `page.page_size`（均 > 0）
  - `state?: string`
  - `sync_mode?: string`
  - `retryable?: boolean`
- Response:
  - `items[]`:
    - `id`（优先 `request_id`，旧数据回退 `job_runs.id`）
    - `state`
    - `error_code`
    - `created_at`
    - `success_count`
    - `fail_count`
    - `pipeline_stage?`（仅 `publish_audit` 聚合路径）
    - `sync_mode?`（仅 `publish_audit` 聚合路径）
    - `retryable?`（仅 `publish_audit` 聚合路径）
  - `page/page_size/total`
- 筛选参数定义（与现行实现一致）：
  - `state`：字符串精确匹配。推荐值：`success|failed|partial|skipped`（legacy 回退路径还可能命中 `started`）。
  - `sync_mode`：字符串精确匹配。推荐值：`page_tree|database|unknown`。
  - `retryable`：布尔匹配（`true|false`）。
  - 空字符串会被当作未传（服务端 trim 后忽略）。
  - 非法值不会报错，返回空结果（`items=[]`）。
- legacy 回退语义（必须遵守）：
  1. 先判断时间窗口内是否存在 `publish_audit` 分组数据（按 `request_id` 聚合，忽略筛选条件）。
  2. 若存在分组数据：仅走 `publish_audit` 聚合路径并应用 `state/sync_mode/retryable` 筛选。
  3. 若不存在分组数据：
     - 当传入 `sync_mode` 或 `retryable`：直接返回空结果（`total=0`），不猜测旧数据语义。
     - 否则回退 `job_runs(job_type=notion_sync_once)`，仅应用 `state` 筛选。
  4. legacy 回退返回项固定 `pipeline_stage=null`、`sync_mode=null`、`retryable=null`。

### `mark_source_inactive({ item_id })`
- Request:
  - `item_id`（非空，且 source item 必须存在）
- Response:
  - `item_id`
  - `active_state`（固定 `inactive`）
  - `updated_at`
- 行为约束：
  - `source_state` upsert 为 `inactive`
  - 对应 `publish_tasks` 改为 `ignored/source_inactive`
  - 对应 `sync_records(pending|retry)` 改为 `failed/source_inactive`

## Error Code Baseline
- `IPC-6001`: 参数/配置校验失败（如缺失必填 env、range 为空）
- `DB-4001`: 数据库存储失败
- `INT-9000`: 内部异常
- `MOD-3002`: 同步模块失败（写入 `sync_records.last_error_code` / `publish_tasks.error_code`）
- `MOD-3429`: Notion 限流/429（写入 `sync_records.last_error_code` / `publish_tasks.error_code`）

## B2-G1 Routing And Directory Contract

### Target Directory Structure (page_tree mode)
1. Root：`NOTION_ROOT_PAGE_ID` 指向唯一根页面（建议标题 `Personal`）。
2. Category：根页面下一级，目录键为 `category_name`（大小写不敏感匹配）。
3. Week：分类页面下一级，目录键为 `week_key`（格式 `YYYY-Www`）。
4. Item：周页面下一级，目录键为 `normalized_item_id` 对应的内容页。

### Category Set Baseline
1. 默认分类集合（必须完整保留）：
   - `学习`
   - `工作`
   - `生活`
   - `健康`
   - `财务`
   - `灵感`
   - `Inbox`
2. unknown 分类：
   - `notion.tree.unknown_category` 默认 `Inbox`。
3. 分类来源：
   - 环境变量 `NOTION_TREE_DEFAULT_CATEGORIES` 优先于 `app_config.notion.tree.default_categories`。

### growth_limit Strategy + Fallback Semantics
1. 规则参数：
   - `route_min_confidence` 默认 `0.72`。
   - `category_growth_limit` 默认 `32`，且必须 `>= 0`。
2. 路由语义：
   - `confidence < route_min_confidence`：强制路由到 unknown，`route_reason=low_confidence`。
   - 候选分类为默认集合：直接命中默认分类，`route_reason=null`。
   - 候选分类为自定义分类且 `growth_limit=0`：路由到 unknown，`route_reason=growth_limit_exceeded`。
   - 候选分类为自定义分类且已达上限：路由到 unknown，`route_reason=growth_limit_exceeded`。
   - 候选分类为自定义分类且未达上限：允许创建新分类页。
3. 回退语义：
   - 参数非法（如负数 growth_limit、非法 timezone）必须返回 `IPC-6001`，不允许静默回退。
   - page_tree 模式缺失 `NOTION_ROOT_PAGE_ID` 必须返回 `IPC-6001`。

### Root Page Check Requirement
`NOTION_ROOT_PAGE_ID` 的上线前检查必须通过：
- `/Users/oliver/Documents/helper/.ops/context/notion-root-page-checklist-v1.md`

## B2-G2 Structured Content Contract (Merge Gate)

### Structured Payload Schema
说明：该 schema 用于发布执行内部载荷与验收，不改变 IPC envelope 结构。

```json
{
  "summary": "string, required, 1..5000",
  "key_points": ["string, optional, each 1..200, max 10"],
  "tags": ["string, optional, each 1..50, max 20"],
  "images": [
    {
      "url": "https://...",
      "alt": "string, optional, max 120"
    }
  ],
  "route_reason": "string|null (null|low_confidence|growth_limit_exceeded)"
}
```

### Result Classes (success / degraded / failed)
1. Success：`summary` 非空，结构化字段合法，Notion 写入成功。
2. Degraded：Notion 写入成功，但 `key_points/tags/images` 任一为空或被裁剪；`summary` 必须保留。
3. Failed：结构化字段非法或写入失败，返回受控错误码并进入重试/死信策略。

### JSON Examples

#### Structured success
```json
{
  "status": "success",
  "payload": {
    "summary": "这条内容讲述了如何搭建每周知识整理流程。",
    "key_points": [
      "先统一来源去重",
      "低置信内容进入审核队列",
      "审核通过后发布到 Notion 页面树"
    ],
    "tags": ["workflow", "notion", "weekly"],
    "images": [
      {
        "url": "https://cdn.example.com/cover.png",
        "alt": "流程示意图"
      }
    ],
    "route_reason": null
  }
}
```

#### Structured degraded
```json
{
  "status": "degraded",
  "payload": {
    "summary": "置信度不足，条目已降级路由到 Inbox。",
    "key_points": [],
    "tags": ["inbox"],
    "images": [],
    "route_reason": "low_confidence"
  },
  "degraded_fields": ["key_points", "images"]
}
```

#### Structured failed
```json
{
  "status": "failed",
  "error": {
    "code": "IPC-6001",
    "message": "structured payload validation failed: summary must not be empty",
    "retryable": false
  }
}
```

## Compatibility Window
- 旧 IPC `collect/get_review_items/run_notion_sync_once` 保留一个迭代周期。
- 前端优先调用新 IPC；若命令不存在，允许回退到旧路径（由 `desktop/src/services/ipc.ts` 适配层兜底）。
- 结构化字段兼容窗口（固定日期）：
  - 2026-02-26 至 2026-03-31：允许“旧字段 + 新结构化载荷”并存。
  - 旧字段写入保留周期到 2026-03-31（`analysis_results.summary_text`、`classification_json`、metadata 文本段）。
  - 自 2026-04-01 起：新增 B2 变更若未携带结构化载荷 schema，Gate B 直接判定不通过。

## JSON Examples

### `get_publish_queue` success
```json
{
  "ok": true,
  "request_id": "e4f1dd7c-53d8-4f9d-846f-9f7de7cd5ef9",
  "data": {
    "items": [
      {
        "id": "pt_src_abc",
        "item_id": "src_abc",
        "title": "CMU 一年级 PhD 学期小结",
        "state": "failed",
        "attempt_count": 2,
        "next_retry_at": "2026-02-27 10:20:00",
        "error_code": "MOD-3002",
        "updated_at": "2026-02-27 10:10:00"
      }
    ],
    "page": 1,
    "page_size": 20,
    "total": 1
  },
  "error": null
}
```

### `get_publish_history` success
```json
{
  "ok": true,
  "request_id": "95fd0420-9580-45b6-95f5-2efb6a4f3c2e",
  "data": {
    "items": [
      {
        "id": "pub_9be2d6ef-c7cb-4108-b7d2-75aa2e99b6ec",
        "state": "partial",
        "error_code": "MOD-3002",
        "created_at": "2026-02-27 10:10:01",
        "success_count": 18,
        "fail_count": 2,
        "pipeline_stage": "publish_approved_to_notion",
        "sync_mode": "page_tree",
        "retryable": true
      }
    ],
    "page": 1,
    "page_size": 20,
    "total": 3
  },
  "error": null
}
```

### `get_publish_history` success (legacy fallback)
```json
{
  "ok": true,
  "request_id": "a6b50df3-063a-4f93-bc09-cc8f4048cb4f",
  "data": {
    "items": [
      {
        "id": "jr_20260226_001",
        "state": "failed",
        "error_code": "MOD-3429",
        "created_at": "2026-02-26 10:00:12",
        "success_count": 0,
        "fail_count": 1,
        "pipeline_stage": null,
        "sync_mode": null,
        "retryable": null
      }
    ],
    "page": 1,
    "page_size": 20,
    "total": 1
  },
  "error": null
}
```

### `publish_approved_to_notion` success
```json
{
  "ok": true,
  "request_id": "f3e4dc53-0562-4db9-a5d8-4f8aa197d8eb",
  "data": {
    "request_id": "pub_77e5f2e1-4e84-4dc6-93f5-1f13d6f2f9a8",
    "attempted": 20,
    "succeeded": 18,
    "failed": 2,
    "requeued": 2,
    "dead_lettered": 0
  },
  "error": null
}
```

### `publish_approved_to_notion` error (page_tree 缺 root)
```json
{
  "ok": false,
  "request_id": "11dc608a-78a4-4ed0-8fc0-1d6fd54f0d13",
  "data": null,
  "error": {
    "code": "IPC-6001",
    "message": "validation failed: NOTION_ROOT_PAGE_ID is required",
    "retryable": false
  }
}
```

### `mark_source_inactive` success
```json
{
  "ok": true,
  "request_id": "01c98aa9-49f0-4458-bf0b-78f9ebda3150",
  "data": {
    "item_id": "src_abc",
    "active_state": "inactive",
    "updated_at": "2026-02-27 10:21:03"
  },
  "error": null
}
```

### Generic validation error
```json
{
  "ok": false,
  "request_id": "359c3229-0e70-4db8-aab8-cf6efe1000a1",
  "data": null,
  "error": {
    "code": "IPC-6001",
    "message": "validation failed: range.from and range.to must not be empty",
    "retryable": false
  }
}
```

### `get_publish_history` error (invalid page)
```json
{
  "ok": false,
  "request_id": "0a76bd15-e2df-49a5-9589-4f5be8f45a53",
  "data": null,
  "error": {
    "code": "IPC-6001",
    "message": "page and page_size must be positive",
    "retryable": false
  }
}
```

### `get_publish_history` error (missing DB env)
```json
{
  "ok": false,
  "request_id": "de7cd04b-66ea-449a-b14c-9af57894e345",
  "data": null,
  "error": {
    "code": "IPC-6001",
    "message": "HELPER_DB_PATH is required",
    "retryable": false
  }
}
```

### `publish_audit` row example (DB observation)
```json
{
  "id": "pa_7a4cf7d2-4d9d-4baf-b090-7b79f55db5ad",
  "publish_task_id": "pt_src_abc",
  "item_id": "src_abc",
  "request_id": "pub_77e5f2e1-4e84-4dc6-93f5-1f13d6f2f9a8",
  "status": "failed",
  "latency_ms": 182,
  "error_code": "MOD-3429",
  "error_message": "notion create child page non-success 429: rate_limited",
  "pipeline_stage": "publish_approved_to_notion",
  "sync_mode": "page_tree",
  "retryable": 1,
  "created_at": "2026-02-27 11:12:45"
}
```
