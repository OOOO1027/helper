# Notion Root Page Checklist v1

Status: ACTIVE  
Owner: Backend Lead + QA  
Last updated: 2026-02-26

## Purpose

确保 `NOTION_ROOT_PAGE_ID` 指向正确、可写、可回滚的页面树根节点。  
未通过本清单，禁止执行 B2 发布链路验收与合并。

## Pre-check

- [ ] 已确认运行模式为 `NOTION_SYNC_MODE=page_tree`。
- [ ] 当前环境已注入：`NOTION_TOKEN`、`NOTION_ROOT_PAGE_ID`、`HELPER_DB_PATH`、`HELPER_DB_KEY`。
- [ ] Notion Integration 已在目标根页面执行 Add connections 授权。

## Root ID Validity

- [ ] `NOTION_ROOT_PAGE_ID` 为非空字符串。
- [ ] ID 规范化后长度为 32 位十六进制（忽略连字符）。
- [ ] Root 页面在 Notion 中可打开且不是 archived 状态。

## Write Permission

- [ ] 使用当前 token 可读取 root 子页面列表（无 401/403）。
- [ ] 可在 root 下创建测试分类页并删除（或标记归档）。
- [ ] 失败时返回受控错误（`IPC-6001`/`MOD-3002`/`MOD-3429`），不出现 silent failure。

## Routing Sanity

- [ ] 低置信样本路由到 unknown 分类（默认 `Inbox`），并带 `route_reason=low_confidence`。
- [ ] `category_growth_limit=0` 时自定义分类样本路由到 unknown，`route_reason=growth_limit_exceeded`。
- [ ] 默认分类样本写入对应分类页，不应被回退到 unknown。

## Evidence Attachments

- [ ] 根页面 URL 与 `NOTION_ROOT_PAGE_ID` 映射截图（遮蔽敏感信息）。
- [ ] 一次发布 run 的 `request_id` 与 `get_publish_history` 记录截图。
- [ ] 一条成功样本与一条回退样本在 Notion 页面树中的位置截图。
- [ ] 异常样本日志摘要（错误码 + message + retryable）。

## Gate Decision

- [ ] 通过：允许进入 Gate B 的 B2 场景验收。
- [ ] 不通过：回退到配置修复，不允许合并 B2 相关变更。
