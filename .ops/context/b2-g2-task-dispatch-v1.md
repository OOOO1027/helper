# B2-G2 技术任务分发单 v1

## Objective
在不破坏 B2-G1 闭环的前提下，实现 `App 内 AI 结构化生成 -> 质量门禁 -> Notion 发布`。

## Global Constraints
1. Root page 固定为 `Personal` 根页本身。
2. Notion 是最终呈现面，App 是处理引擎。
3. 预算策略：`<100 RMB / month`，超预算仅降处理量，不降质量门槛。
4. v1 来源固定 `xhs`，不引入新来源。

## Thread A - Backend（主实现）
### 任务
1. 新增结构化生成应用服务：`summary/key_points/tags/degraded_fields`。
2. 接入模型调用抽象层（provider 可替换），默认走 App 内模型。
3. 在发布前增加质量评分器（见 `b2-g2-quality-rubric-v1.md`）。
4. 将质量状态写入发布审计：`success/degraded/failed` 与原因。
5. 降级策略：模型失败时可回退规则摘要，但状态必须标记 `degraded`。
6. 保持 IPC 契约兼容，不改 envelope。

### 验收标准
1. 10 条样本至少 8 条达到 `>=75`。
2. 每条都有可追踪评分明细与 degraded/failed 原因。
3. Notion 页面字段落地一致：`summary/key_points/tags/route_reason/images`。
4. `cargo test -q` 全绿。

### 提示词（可直接下发）
你是 Backend 线程，执行 B2-G2 主实现。  
目标：把当前启发式摘要升级为“App 内 AI + 质量门禁”的可观测发布链路。  
必须遵守：
1. 不改 IPC envelope；
2. 所有状态受控（success/degraded/failed）；
3. 失败可追踪（error_code + request_id + degraded_fields）；
4. 默认分类与 growth_limit 基线保持现有实现。  
交付：
1. 代码改动清单；
2. 新增/修改测试；
3. 10 条样本验收记录（JSON + 评分）。

## Thread B - Frontend（展示与回路）
### 任务
1. 发布日志详情卡增加质量信息展示：
   - `quality_score`
   - `degraded_fields`
   - `route_reason`
2. 审核抽屉增加“结构化预览”：
   - 结论摘要
   - 关键要点
   - 标签
   - 图像摘要（封面+前3图）
3. 状态文案统一：
   - success：可直接查看 Notion
   - degraded：可发布但建议复核
   - failed：未发布，提供重试入口
4. 失败态必须给“发生了什么 + 下一步动作”。

### 验收标准
1. 发布中心一屏可判断“是否值得信任”。
2. 相同状态只允许一套视觉/文案写法。
3. `pnpm build` 通过，关键交互可键盘可达。

### 提示词（可直接下发）
你是 Frontend 线程，执行 B2-G2 可视化改造。  
目标：让用户在发布前后都能看到结构化质量，而不是只看成功/失败计数。  
必须遵守：
1. 一屏一个主动作；
2. 同状态单一视觉规范；
3. 不引入新页面层级，只增强现有发布中心/审核抽屉。  
交付：
1. UI 改动说明；
2. 核心截图（审核预览 + 发布日志详情）；
3. 构建与类型检查结果。

## Thread C - 文档监工（合同与门禁）
### 任务
1. 将 `b2-g2-quality-rubric-v1.md` 引入 Gate B 文档。
2. 更新 `b2-acceptance-template-v1.md`，新增评分明细区。
3. 在 `publish-ipc-contract-v1.md` 补充质量状态字段示例。
4. 新增一条 decision-log，记录“App 内 AI 主链路”决策。

### 验收标准
1. 合同、ADR、Gate 文档三处口径一致。
2. 不再出现“默认值/状态定义”冲突。
3. 验收模板可直接用于本轮 10 条样本。

### 提示词（可直接下发）
你是文档监工线程，执行 B2-G2 文档门禁收口。  
目标：把“质量要求”从口头约束变成可打勾验收项。  
必须遵守：
1. 只写与实现一致的事实；
2. 所有新增规则必须有对应验收字段；
3. 不引入与代码冲突的默认值。  
交付：
1. 更新文档路径清单；
2. 变更摘要；
3. 门禁自检结果（通过/阻塞项）。

## Integration Checkpoints（总线验收）
1. CP1（Backend done）：后端结构化链路 + 测试全绿。
2. CP2（Frontend done）：发布中心可见质量详情，构建通过。
3. CP3（Docs done）：合同/ADR/Gate 一致且可执行。
4. CP4（Joint run）：10 条真实样本闭环验收。

## Final Gate Rule
未满足以下任一，B2-G2 不放行：
1. 样本通过率达标（>=8/10）
2. degraded/failed 可追踪
3. Notion 呈现字段完整
