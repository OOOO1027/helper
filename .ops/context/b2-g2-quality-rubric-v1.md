# B2-G2 Structured Content Quality Rubric v1

## Scope
用于 `App 内 AI 生成 -> Notion 发布` 的结构化内容质量门禁。  
适用来源：`xhs`（v1 唯一来源）。

## Hard Decisions
1. 主生成引擎固定为 App 内 AI；Notion AI 仅允许“发布后人工二次润色”，不得作为主链路。
2. 若预算触顶，执行“保质量降处理量”，不得降级到“无结构化直接发布”。
3. 未通过质量门禁的条目进入 `degraded` 或 `failed`，不得伪装为 `success`。

## Output Contract (B2-G2)
必须产出以下字段：
1. `summary` (required)
2. `key_points` (optional but scored)
3. `tags` (optional but scored)
4. `images` (optional but scored, 来源于抓取字段)
5. `route_reason` (required by routing contract, nullable)

## Writing Style Baseline (from product owner sample)
1. 先给“核心方法论/核心结论”，再展开行动框架。
2. 关键内容以编号结构呈现（2-4层级，不堆砌）。
3. 每个行动框架必须可执行，至少包含 1 个可落地动作。
4. 不做空泛鸡汤，不堆“正确废话”。
5. 高价值内容允许更长摘要；娱乐内容允许短摘要。

## Quality Score (100)
1. 结构完整性（25）
   - `summary` 非空且语义完整（10）
   - `key_points` >= 3 且非重复（10）
   - `tags` 3-8 个且非“未分类”灌水（5）
2. 信息忠实度（25）
   - 与原文事实一致，不捏造数字/结论（15）
   - 关键判断有原文依据（10）
3. 可执行性（20）
   - 给出明确动作或决策框架（10）
   - 能回答“我下一步该做什么”（10）
4. 信息密度与可读性（20）
   - 无明显冗词，分段清楚（10）
   - 重点突出，主次分明（10）
5. 路由与分类一致性（10）
   - `final_category` 与内容一致（5）
   - `route_reason` 与策略一致（5）

## Pass Line
1. `score >= 75` -> `success`
2. `55 <= score < 75` -> `degraded`（必须写入 `degraded_fields`）
3. `score < 55` -> `failed`

## Fail Fast Rules
任一命中直接 `failed`：
1. `summary` 为空或仅重复标题。
2. `key_points` 全为空或全重复。
3. 事实冲突（例如来源未提及却生成具体数字/事件）。
4. 输出不符合 JSON schema。

## Degraded Rules
满足以下任一：
1. `summary` 合格，但 `key_points` 不足 3 条。
2. `tags` 少于 2 或大量“未分类/泛标签”。
3. 图片缺失时未标记来源限制。

## Prompt Baseline (for backend model call)
系统提示词（核心约束）：
1. 你是知识提炼引擎，输出必须是 JSON，不要输出 Markdown。
2. 优先提炼“结论 + 行动框架 + 可执行步骤”。
3. 不得编造原文不存在的信息；不确定就写“信息不足”。
4. `summary` 中文，80-260 字；`key_points` 3-6 条；`tags` 3-8 个。
5. 输出字段固定：`summary,key_points,tags,route_reason,degraded_fields`。

## Example Use (xhs)
示例输入：
`蒸馏了一位AI创业者，这是学到的东西...`

示例期望：
1. `summary`：提炼“成长路径 + 决策原则 + 关系策略”的主结论。
2. `key_points`：至少覆盖“可视化成长、稀缺性设计、规则协商、长期主义”。
3. `tags`：如 `成长策略/职业决策/长期主义/创业思维`。
4. `route_reason`：按路由策略给出 `null/low_confidence/growth_limit_exceeded`。

## Acceptance Dataset
Gate B2-G2 至少 10 条真实样本，分布：
1. 高价值深内容 4 条
2. 中等信息密度 4 条
3. 低信息/娱乐 2 条

每条需留痕：
1. 原文片段
2. 结构化输出 JSON
3. 评分明细
4. 最终状态（success/degraded/failed）
