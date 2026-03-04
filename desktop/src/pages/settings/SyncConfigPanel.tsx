import { RemoteState } from "../../components/RemoteState";
import type { SyncConfigSnapshot } from "../../types/contracts";
import { modeLabel, statusLabel } from "./settingsHelpers";

interface Props {
  syncConfig: SyncConfigSnapshot | null;
  syncConfigLoading: boolean;
  syncConfigError: string | null;
  syncModeUnknown: boolean;
  rootPageMissing: boolean;
  databaseIdMissing: boolean;
  tokenMissing: boolean;
  actionBusy: boolean;
  onRefresh: () => void;
}

export function SyncConfigPanel({
  syncConfig,
  syncConfigLoading,
  syncConfigError,
  syncModeUnknown,
  rootPageMissing,
  databaseIdMissing,
  tokenMissing,
  actionBusy,
  onRefresh,
}: Props) {
  return (
    <article className="panel-card">
      <h3>同步配置状态</h3>
      <p className="hint">用于预判"立即同步一次"是否可执行，避免点了才失败。</p>
      <RemoteState
        loading={syncConfigLoading}
        error={syncConfigError}
        empty={!syncConfig}
        loadingText="同步配置加载中..."
        emptyText="暂未获取到同步配置快照。"
        errorNextStep="下一步：点击「刷新配置状态」；若持续失败，请确认后端配置快照 IPC 已部署。"
        emptyNextStep="下一步：点击「刷新配置状态」，确认模式与配置项后再执行同步。"
        onRetry={onRefresh}
        retryLabel="刷新配置状态"
      >
        <div className="kv-row">
          <span>当前模式</span>
          <code>{modeLabel(syncConfig?.mode ?? "unknown")}</code>
        </div>
        <div className="kv-row">
          <span>Notion 凭证</span>
          <code>{statusLabel(syncConfig?.token_configured ?? null)}</code>
        </div>
        <div className="kv-row">
          <span>database id</span>
          <code>{statusLabel(syncConfig?.database_id_configured ?? null)}</code>
        </div>
        <div className="kv-row">
          <span>root page id</span>
          <code>{statusLabel(syncConfig?.root_page_id_configured ?? null)}</code>
        </div>
        <div className="kv-row">
          <span>timezone</span>
          <code>
            {syncConfig?.timezone && syncConfig.timezone.length > 0 ? syncConfig.timezone : "-"}
          </code>
        </div>
        <div className="kv-row">
          <span>category growth limit</span>
          <code>{syncConfig?.category_growth_limit ?? "-"}</code>
        </div>
        {syncConfig?.source === "fallback" && (
          <div className="state-panel warning" role="status" aria-live="polite">
            当前运行时尚未返回配置快照，前置校验会按最小可用策略执行。
            <small>下一步：建议升级后端到包含配置快照 IPC 的版本，获得完整预检能力。</small>
          </div>
        )}
        {syncConfig?.source === "ipc" && syncModeUnknown && (
          <div className="state-panel warning" role="status" aria-live="polite">
            当前同步模式配置无效，手动同步会被拦截。
            <small>
              下一步：将 NOTION_SYNC_MODE 设为 page_tree 或 database，再点击「刷新配置状态」。
            </small>
          </div>
        )}
        {rootPageMissing && (
          <div className="state-panel warning" role="status" aria-live="polite">
            当前为页面树模式，但未配置「同步起始页面」，本次同步会被拦截。
            <small>下一步：在环境配置中补齐 root page id，再回到此页重试。</small>
          </div>
        )}
        {databaseIdMissing && (
          <div className="state-panel warning" role="status" aria-live="polite">
            当前为数据库模式，但未配置「目标数据库」，本次同步会被拦截。
            <small>下一步：在环境配置中补齐 database id，再回到此页重试。</small>
          </div>
        )}
        {tokenMissing && (
          <div className="state-panel warning" role="status" aria-live="polite">
            当前未配置 Notion 凭证，本次同步会被拦截。
            <small>下一步：先补齐凭证，再执行手动同步或失败回放。</small>
          </div>
        )}
        <div className="inline-actions">
          <button type="button" onClick={onRefresh} disabled={actionBusy || syncConfigLoading}>
            {syncConfigLoading ? "刷新中..." : "刷新配置状态"}
          </button>
        </div>
      </RemoteState>
    </article>
  );
}
