# B2-G2 真实联调操作脚本 v1

## 0) 前提
1. 已有可用 `XHS_COOKIE` 或 `XHS_COOKIE_FILE`。
2. 已获取 `QWEN_API_KEY`。
3. 本次目标：验证 `App 内 AI 生成 -> 质量门禁(75/55) -> Notion 发布`。

## 1) 单次会话环境变量（推荐先这样跑）
```bash
export QWEN_API_KEY="你的真实key"
export QWEN_MODEL="qwen-flash"
export XHS_COOKIE_FILE="/Users/oliver/.helper/xhs.cookie"

export B2_G2_AI_ENABLED=1
export B2_G2_XHS_DEEP_FETCH=1
export B2_G2_QWEN_MAX_CALLS_PER_RUN=10
export B2_G2_XHS_DEEP_FETCH_TIMEOUT_MS=8000
```

## 2) 快速健康检查
```bash
zsh -lc 'echo "QWEN_API_KEY len=${#QWEN_API_KEY}"; echo "QWEN_MODEL=${QWEN_MODEL}"; [ -f "$XHS_COOKIE_FILE" ] && echo "XHS_COOKIE_FILE ok" || echo "XHS_COOKIE_FILE missing"'
```

## 3) 启动桌面联调
```bash
cd /Users/oliver/Documents/helper/desktop
pnpm tauri dev
```

## 4) 应用内联调路径（手动）
1. 收件箱：触发小红书增量抓取。
2. 审核：通过 10 条样本（至少覆盖 1 条深内容）。
3. 发布中心：点击立即发布（limit=10）。
4. 发布日志：展开最新请求，确认以下字段有值：
   - `conclusion_summary`
   - `key_points`
   - `tags`
   - `quality_state`
   - `quality_score`
   - `degraded_fields`
5. Notion：在 `Personal/{分类}/{ISO周}` 下核对页面结构和内容区块。

## 5) 验收通过标准
1. 10 条样本中 `success >= 8`。
2. `degraded/failed` 条目都有明确原因（`degraded_fields/error_code`）。
3. 页面必须包含：结论摘要、关键要点、原文链接、标签、状态元信息、图片块。

## 6) 常见故障与处理
1. `QWEN_API_KEY` 未设置：会退化为规则摘要，质量下降。
2. 小红书正文抽不到：检查 `XHS_COOKIE_FILE` 是否过期，更新后重试。
3. 成本偏高：下调 `B2_G2_QWEN_MAX_CALLS_PER_RUN` 到 `5`。
