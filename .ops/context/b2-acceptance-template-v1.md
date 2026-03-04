# B2 Acceptance Template v1

Status: TEMPLATE  
Owner: FE Lead + BE Lead + QA  
Last updated: 2026-02-26

## Basic Info

- 验收日期：`YYYY-MM-DD`
- 分支/提交：`<branch-or-commit>`
- 填报人：`<name>`
- 审核人（Aiden）：`<name>`
- 运行模式：`page_tree | database`

## Environment Snapshot

- `HELPER_DB_PATH`：`configured|missing`
- `HELPER_DB_KEY`：`configured|missing`
- `NOTION_TOKEN`：`configured|missing`
- `NOTION_ROOT_PAGE_ID`：`configured|missing`
- `NOTION_SYNC_MODE`：`page_tree|database`

## B2-G1 Routing & Directory

### Scenario Records

```json
[
  {
    "scenario_id": "B2-G1-01",
    "owner": "backend",
    "input": "publish sample A",
    "expected": "Root->Category->Week->Item",
    "actual": "matched",
    "pass": true,
    "artifacts": [
      "notion_screenshot_path",
      "request_id:pub_xxx"
    ]
  }
]
```

### Root Page Checklist

- [ ] 已附 `/Users/oliver/Documents/helper/.ops/context/notion-root-page-checklist-v1.md` 完整勾选结果。

## B2-G2 Structured Content

### Success / Degraded / Failed Evidence

- [ ] 成功样例已附（含结构化 payload 与 Notion 结果截图）。
- [ ] 降级样例已附（含 degraded_fields 与 route_reason）。
- [ ] 失败样例已附（含错误码、message、retryable）。

### Compatibility Window Check

- [ ] 旧字段读取未破坏（`summary_text/classification_json`）。
- [ ] 新结构化字段验证通过（`summary/key_points/tags/images/route_reason`）。
- [ ] 保留周期检查通过（截至 2026-03-31）。

## B2-G3 Gate Artifacts

- [ ] 前端构建结果已附（`pnpm build`、`pnpm tauri dev`）。
- [ ] 后端回归结果已附（`cargo test -q`）。
- [ ] 发布历史证据已附（`get_publish_history` 记录）。
- [ ] 回滚路径证据已附（`NOTION_SYNC_MODE=database` 一次验证）。

## Risk & Rollback

- 当前风险：
  - `R1: <描述>`
  - `R2: <描述>`
- 回滚触发条件：
  - `C1: <条件>`
  - `C2: <条件>`
- 回滚动作：
  - `A1: <动作>`
  - `A2: <动作>`

## Final Decision

- [ ] Gate B 通过（可合并）
- [ ] Gate B 不通过（阻塞合并）
- 备注（必须具体）：
  - `<一句话结论 + 下一步责任人 + 截止时间>`
