# CodeX 前端改进工作文档

> 面向 CodeX 前端 Thread · 基于 2026-03 架构审查  
> 状态: **待执行** | 优先级标记: 🔴 P0 🟡 P1 🟢 P2

---

## 一、架构总评

当前前端基于 TypeScript + React 18 + Vite 5 + Tauri 2 构建，已完成 4 个主页面
（收件箱、审核、发布中心、系统健康）的功能开发。IPC 适配层设计合理，类型合约完整。
但在状态管理、错误边界、组件拆分和性能优化方面存在系统性短板。

---

## 二、改进任务清单

### 🔴 P0 — 必须修复（影响用户体验 / 应用稳定性）

#### F-P0-01: 增加全局 ErrorBoundary

**现状问题**  
应用没有 React ErrorBoundary，任何组件运行时异常会导致整个应用白屏崩溃，用户只能重启。

**改进方案**  
1. 新建 `desktop/src/components/ErrorBoundary.tsx`：
   ```tsx
   import { Component, type ErrorInfo, type ReactNode } from "react";

   interface Props { children: ReactNode; }
   interface State { hasError: boolean; error: Error | null; }

   export class ErrorBoundary extends Component<Props, State> {
     state: State = { hasError: false, error: null };

     static getDerivedStateFromError(error: Error): State {
       return { hasError: true, error };
     }

     componentDidCatch(error: Error, info: ErrorInfo) {
       console.error("[ErrorBoundary]", error, info.componentStack);
     }

     render() {
       if (this.state.hasError) {
         return (
           <div role="alert" style={{ padding: "2rem", textAlign: "center" }}>
             <h2>应用出现错误</h2>
             <p>{this.state.error?.message}</p>
             <button onClick={() => this.setState({ hasError: false, error: null })}>
               重试
             </button>
           </div>
         );
       }
       return this.props.children;
     }
   }
   ```
2. 在 `App.tsx` 最外层包裹 `<ErrorBoundary>`。
3. 对每个 Page 组件也可按需包裹独立的 ErrorBoundary 以隔离故障域。

**涉及文件**  
- 新建 `desktop/src/components/ErrorBoundary.tsx`
- `desktop/src/App.tsx` — 包裹 ErrorBoundary
- 可选：各 Page 组件

**验收标准**  
- [ ] 组件异常时显示友好错误界面而非白屏
- [ ] 用户可点击"重试"恢复
- [ ] 错误信息输出到 console（方便调试）
- [ ] `pnpm build` 和 `pnpm lint` 通过

---

#### F-P0-02: 消除异步请求内存泄漏

**现状问题**  
页面组件在 `useEffect` 中发起 IPC 请求，但未在 cleanup 中取消。  
快速切换页面时，已卸载组件的 setState 调用会导致内存泄漏和 React 警告。

**改进方案**  
1. 在所有 `useEffect` 数据加载中使用 `AbortController` 或 `ignore` 标志：
   ```tsx
   useEffect(() => {
     let cancelled = false;
     loadData().then((data) => {
       if (!cancelled) setData(data);
     });
     return () => { cancelled = true; };
   }, [deps]);
   ```
2. 提取为通用 hook `useAsyncEffect` 或 `useCancellableRequest`。
3. 对长时间操作（如批量发布），增加 loading 状态和取消按钮。

**涉及文件**  
- 新建 `desktop/src/hooks/useAsyncEffect.ts`（可选）
- `desktop/src/pages/InboxPage.tsx`
- `desktop/src/pages/ReviewQueuePage.tsx`
- `desktop/src/pages/PublishCenterPage.tsx`
- `desktop/src/pages/SystemHealthPage.tsx`

**验收标准**  
- [ ] 快速切换页面时无 "setState on unmounted" 警告
- [ ] 所有 useEffect 数据加载有 cleanup 处理
- [ ] `pnpm build` 通过

---

### 🟡 P1 — 应该改进（影响可维护性 / 开发效率）

#### F-P1-01: 引入轻量状态管理

**现状问题**  
- 无全局状态容器，状态通过 props 逐层传递
- `App.tsx` 通过 `onNotify` 回调向下传递通知方法
- 各页面独立管理 loading/error/data 状态，逻辑重复
- 跨页面状态共享困难（如：收件箱导入后审核队列需刷新）

**改进方案**  
1. 引入 Zustand（零配置、TypeScript 原生、体积小 ~1KB）：
   ```tsx
   // desktop/src/stores/useAppStore.ts
   import { create } from "zustand";

   interface AppState {
     notices: NoticeItem[];
     pushNotice: (n: Omit<NoticeItem, "id">) => void;
     removeNotice: (id: string) => void;
     reviewRefreshToken: number;
     triggerReviewRefresh: () => void;
   }
   ```
2. 将通知系统从 `App.tsx` 迁移到全局 store。
3. 跨页面信号（如导入完成 → 刷新审核）通过 store 传递。

**涉及文件**  
- 新建 `desktop/src/stores/useAppStore.ts`
- `desktop/src/App.tsx` — 简化，移除 prop drilling
- 各 Page 组件 — 从 store 读取/写入状态
- `package.json` — 添加 `zustand` 依赖

**验收标准**  
- [ ] 通知系统通过 store 管理，不再 prop drilling
- [ ] 跨页面状态同步正常（导入→审核→发布）
- [ ] 现有交互行为不变
- [ ] Bundle 体积增加 < 2KB gzip

---

#### F-P1-02: 拆分大型页面组件

**现状问题**  
`ReviewQueuePage.tsx` 约 800+ 行，包含了数据加载、过滤、排序、批量操作、抽屉显示等全部逻辑。  
`PublishCenterPage.tsx` 和 `InboxPage.tsx` 也存在类似问题。

**改进方案**  
1. 提取自定义 hooks 封装数据逻辑：
   ```
   hooks/
   ├── useReviewQueue.ts      — 审核队列数据 + 分页 + 过滤
   ├── usePublishCenter.ts    — 发布队列 + 历史
   ├── useInboxData.ts        — 收件箱数据 + 导入
   └── useSystemHealth.ts     — 系统健康指标
   ```
2. 提取 UI 子组件：
   ```
   components/review/
   ├── ReviewToolbar.tsx       — 过滤 + 排序控件
   ├── ReviewList.tsx          — 列表渲染
   ├── ReviewBatchActions.tsx  — 批量操作
   └── ReviewStats.tsx         — 会话效率指标
   ```
3. 页面组件只做组合：
   ```tsx
   export function ReviewQueuePage() {
     const queue = useReviewQueue();
     return (
       <div>
         <ReviewToolbar {...queue.filters} />
         <ReviewList items={queue.items} />
         <ReviewBatchActions selected={queue.selected} />
       </div>
     );
   }
   ```

**涉及文件**  
- 新建 `desktop/src/hooks/` 目录 + 4 个 hook 文件
- 新建/扩展 `desktop/src/components/review/` 子组件
- `desktop/src/pages/ReviewQueuePage.tsx` — 重构为组合
- `desktop/src/pages/PublishCenterPage.tsx` — 类似重构
- `desktop/src/pages/InboxPage.tsx` — 类似重构

**验收标准**  
- [ ] 各页面组件 < 200 行
- [ ] 自定义 hook 可独立被其他组件复用
- [ ] 功能和视觉效果与改动前一致

---

#### F-P1-03: IPC 层增加请求去重与缓存

**现状问题**  
- 每次页面切换或刷新都发起完整 IPC 调用
- 无请求去重（同一时间可能发出多个相同请求）
- 无短时缓存（快速来回切换时重复加载数据）
- 部分 `.catch(() => {})` 静默吞掉错误

**改进方案**  
1. 在 `services/ipc.ts` 中增加请求去重层：
   ```typescript
   const inflight = new Map<string, Promise<unknown>>();
   
   async function deduped<T>(key: string, fn: () => Promise<T>): Promise<T> {
     if (inflight.has(key)) return inflight.get(key) as Promise<T>;
     const promise = fn().finally(() => inflight.delete(key));
     inflight.set(key, promise);
     return promise;
   }
   ```
2. 对只读查询增加 TTL 缓存（如仪表盘 30 秒内不重复查询）。
3. 消除所有 `.catch(() => {})` 静默处理，改为 `.catch(e => console.warn(e))`。

**涉及文件**  
- `desktop/src/services/ipc.ts` — 增加去重和缓存逻辑
- 各页面 `.catch(() => {})` 调用点

**验收标准**  
- [ ] 同一请求同时只有一个在进行
- [ ] 只读查询有短时缓存（可配置 TTL）
- [ ] 无静默错误吞没
- [ ] 写操作（审核、发布）不缓存

---

#### F-P1-04: 完善无障碍访问 (a11y)

**现状问题**  
- 抽屉组件 (`ReviewDrawer`) 缺少焦点捕获（focus trap）
- 部分按钮缺少 `aria-label`（仅有图标无文字）
- 表单输入缺少关联的 `<label>`
- 键盘导航在批量操作区域不完整

**改进方案**  
1. 为 `ReviewDrawer` 增加焦点捕获：
   - 打开时自动聚焦到抽屉内第一个可交互元素
   - Tab 循环限制在抽屉内
   - Esc 键关闭抽屉
2. 审查所有按钮，为纯图标按钮添加 `aria-label`。
3. 确保所有表单控件有关联的 `<label>` 或 `aria-label`。
4. 增加 `role="dialog"` 和 `aria-modal="true"` 到抽屉组件。

**涉及文件**  
- `desktop/src/components/ReviewDrawer.tsx`
- 各 Page 组件中的按钮和表单
- `desktop/src/components/AppShell.tsx` — 导航区域键盘支持

**验收标准**  
- [ ] 抽屉打开/关闭有正确的焦点管理
- [ ] 所有可交互元素可通过键盘操作
- [ ] 纯图标按钮都有 aria-label
- [ ] 抽屉有正确的 ARIA 角色

---

### 🟢 P2 — 建议改进（提升性能和开发体验）

#### F-P2-01: 路由懒加载与代码分割

**现状问题**  
所有页面在初始加载时一次性打包，增大首屏体积。

**改进方案**  
```tsx
// App.tsx
const InboxPage = lazy(() => import("./pages/InboxPage"));
const ReviewQueuePage = lazy(() => import("./pages/ReviewQueuePage"));
const PublishCenterPage = lazy(() => import("./pages/PublishCenterPage"));
const SystemHealthPage = lazy(() => import("./pages/SystemHealthPage"));

// 使用 Suspense 包裹
<Suspense fallback={<PageSkeleton />}>
  {active === "inbox" && <InboxPage ... />}
</Suspense>
```

**涉及文件**  
- `desktop/src/App.tsx`
- 新建 `desktop/src/components/PageSkeleton.tsx`

**验收标准**  
- [ ] Vite 构建生成多个 chunk
- [ ] 首次加载只加载当前页面的代码
- [ ] 页面切换有平滑的 loading 占位

---

#### F-P2-02: 长列表虚拟化

**现状问题**  
审核队列和发布历史可能包含数百条记录，目前全部渲染 DOM 节点。

**改进方案**  
1. 引入 `@tanstack/react-virtual` 对审核列表和发布历史实施虚拟滚动。
2. 仅在数据量 > 50 条时启用虚拟化，少量数据时保持简单渲染。

**涉及文件**  
- `desktop/src/pages/ReviewQueuePage.tsx`（或重构后的 `ReviewList.tsx`）
- `desktop/src/pages/PublishCenterPage.tsx`
- `package.json` — 添加 `@tanstack/react-virtual` 依赖

**验收标准**  
- [ ] 500 条记录时页面流畅（60fps 滚动）
- [ ] 虚拟化对少量数据无视觉差异

---

#### F-P2-03: IPC 响应运行时校验

**现状问题**  
IPC 响应仅依赖 TypeScript 编译时类型检查。  
后端返回不符合预期的数据时，前端可能出现隐晦的运行时错误。

**改进方案**  
1. 引入 Zod 对关键 IPC 响应做运行时校验：
   ```typescript
   import { z } from "zod";

   const DashboardMetricsSchema = z.object({
     today_collected: z.number(),
     pending_review: z.number(),
     classification_accuracy: z.number(),
     summary_usability: z.number(),
     budget_usage_ratio: z.number(),
   });

   // 在 adapter 层校验
   export function parseDashboardMetrics(raw: unknown) {
     return DashboardMetricsSchema.parse(raw);
   }
   ```
2. 校验失败时抛出明确的类型错误，便于排查后端合约变更。

**涉及文件**  
- 新建 `desktop/src/types/schemas.ts`
- `desktop/src/services/adapters.ts` — 增加校验调用
- `package.json` — 添加 `zod` 依赖

**验收标准**  
- [ ] 关键 IPC 响应（仪表盘、审核列表、发布队列）有运行时校验
- [ ] 类型不匹配时抛出清晰的错误消息
- [ ] Bundle 增量 < 15KB gzip

---

#### F-P2-04: 优化 useMemo / useCallback 使用

**现状问题**  
- `App.tsx` 中 `navItems` 用了 `useMemo`，但事件处理器未用 `useCallback`
- 子组件每次渲染都接收新的函数引用，触发不必要的重渲染

**改进方案**  
1. 为所有传递给子组件的回调使用 `useCallback`。
2. 对计算密集型的列表过滤/排序结果使用 `useMemo`。
3. 对纯展示组件添加 `React.memo` 包裹。

**涉及文件**  
- `desktop/src/App.tsx`
- 各 Page 组件
- `desktop/src/components/AppShell.tsx`

**验收标准**  
- [ ] React DevTools Profiler 无不必要的重渲染
- [ ] 关键交互（切换页面、过滤列表）响应 < 16ms

---

## 三、执行顺序建议

```
Phase 1（稳定性）: F-P0-01 → F-P0-02
Phase 2（可维护）: F-P1-02 → F-P1-01 → F-P1-03 → F-P1-04
Phase 3（性能）  : F-P2-01 → F-P2-04 → F-P2-02 → F-P2-03
```

每完成一个任务后执行：
```bash
pnpm lint
pnpm format:check
pnpm build
```

---

## 四、依赖变更清单

| 包名 | 版本 | 用途 | 优先级 |
|------|------|------|--------|
| `zustand` | ^5.x | 轻量状态管理 | P1 |
| `zod` | ^3.x | 运行时类型校验 | P2 |
| `@tanstack/react-virtual` | ^3.x | 长列表虚拟化 | P2 |

> 所有新依赖添加前需通过安全审查（`gh-advisory-database` 检查）。

---

## 五、风险与注意事项

| 风险 | 缓解措施 |
|------|----------|
| F-P1-01 引入 Zustand 增加学习成本 | Zustand API 极简，类 hooks 风格 |
| F-P1-02 重构时可能遗漏交互细节 | 逐个页面重构，每完成一个做完整回归 |
| F-P0-02 取消请求可能影响写操作 | 仅对读操作应用 cancel 逻辑 |
| F-P2-02 虚拟化改变 DOM 结构 | 确保 a11y 属性在虚拟列表中保持正确 |
