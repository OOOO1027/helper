import { useEffect, useMemo, useState } from "react";
import type { NoticeItem } from "../App";
import {
  collectSource,
  getLocalToolStatus,
  importWechatAndPersist,
  pickWechatFiles,
  toUserMessage,
} from "../services/ipc";
import type { ImportPersistSummary, LocalToolStatus } from "../types/contracts";

interface Props {
  onNotify: (item: Omit<NoticeItem, "id">) => void;
  onOpenExceptions: () => void;
  onOpenReview: () => void;
  onOpenSettings: () => void;
  startupToolStatus: LocalToolStatus | null;
}

interface ImportHistoryItem {
  id: string;
  fileCount: number;
  status: "success" | "partial";
  summary: ImportPersistSummary;
  createdAt: string;
}

interface ImportFeedback {
  source: "import" | "xhs";
  level: "success" | "warning" | "error";
  message: string;
  nextStep?: string;
}

const IMPORT_HISTORY_KEY = "helper.import.history";
type ImportJobState = "idle" | "ready" | "importing" | "success" | "partial" | "failed";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function asNumber(value: unknown, fallback = 0): number {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  return fallback;
}

function normalizeHistorySummary(
  input: unknown,
  fallbackBatchId: string
): ImportPersistSummary | null {
  if (!isRecord(input)) {
    return null;
  }

  if ("batch_id" in input && "parsed_ok" in input && "persisted" in input) {
    return {
      batch_id:
        typeof input.batch_id === "string" && input.batch_id.trim().length > 0
          ? input.batch_id
          : fallbackBatchId,
      total: asNumber(input.total),
      parsed_ok: asNumber(input.parsed_ok),
      duplicates: asNumber(input.duplicates),
      parse_failed: asNumber(input.parse_failed),
      persisted: asNumber(input.persisted),
      persist_failed: asNumber(input.persist_failed),
    };
  }

  if ("batchId" in input && "validUrl" in input) {
    const batchId =
      typeof input.batchId === "string" && input.batchId.trim().length > 0
        ? input.batchId
        : fallbackBatchId;
    const total = asNumber(input.total);
    const parsedOk = asNumber(input.validUrl);
    const duplicates = asNumber(input.duplicates);
    const parseFailed = asNumber(input.failed);
    return {
      batch_id: batchId,
      total,
      parsed_ok: parsedOk,
      duplicates,
      parse_failed: parseFailed,
      persisted: Math.max(parsedOk - duplicates - parseFailed, 0),
      persist_failed: 0,
    };
  }

  return null;
}

function loadHistory(): ImportHistoryItem[] {
  try {
    const raw = localStorage.getItem(IMPORT_HISTORY_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed
      .map((entry, index) => {
        if (!isRecord(entry)) {
          return null;
        }
        const rawId =
          typeof entry.id === "string" && entry.id.trim().length > 0
            ? entry.id
            : `history-${index}`;
        const summary = normalizeHistorySummary(entry.summary, rawId);
        if (!summary) {
          return null;
        }
        const failed = summary.parse_failed + summary.persist_failed;
        return {
          id: rawId,
          fileCount: asNumber(entry.fileCount),
          status:
            entry.status === "success" || entry.status === "partial"
              ? entry.status
              : failed > 0
                ? "partial"
                : "success",
          summary,
          createdAt:
            typeof entry.createdAt === "string" && entry.createdAt.trim().length > 0
              ? entry.createdAt
              : new Date().toISOString(),
        } satisfies ImportHistoryItem;
      })
      .filter((item): item is ImportHistoryItem => Boolean(item))
      .slice(0, 20);
  } catch {
    return [];
  }
}

function pickPaths(files: FileList | null): string[] {
  if (!files) {
    return [];
  }
  return Array.from(files)
    .map((file) => {
      const tauriPath = (file as unknown as { path?: string }).path;
      return tauriPath || file.name;
    })
    .filter(Boolean);
}

function missingTools(status: LocalToolStatus): string[] {
  return [!status.textutil ? "textutil" : null, !status.pdftotext ? "pdftotext" : null].filter(
    (item): item is string => Boolean(item)
  );
}

export function ImportCenterPage({
  onNotify,
  onOpenExceptions,
  onOpenReview,
  onOpenSettings,
  startupToolStatus,
}: Props) {
  const [paths, setPaths] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [feedback, setFeedback] = useState<ImportFeedback | null>(null);
  const [summary, setSummary] = useState<ImportPersistSummary | null>(null);
  const [history, setHistory] = useState<ImportHistoryItem[]>(() => loadHistory());
  const [jobState, setJobState] = useState<ImportJobState>("idle");
  const [progress, setProgress] = useState(0);
  const [xhsSyncing, setXhsSyncing] = useState(false);
  const [toolStatus, setToolStatus] = useState<LocalToolStatus | null>(startupToolStatus);
  const activeMissingTools = useMemo(
    () => (toolStatus ? missingTools(toolStatus) : []),
    [toolStatus]
  );

  const canSubmit = useMemo(
    () => paths.length > 0 && !loading && activeMissingTools.length === 0,
    [activeMissingTools.length, loading, paths.length]
  );

  useEffect(() => {
    if (startupToolStatus) {
      setToolStatus(startupToolStatus);
    }
  }, [startupToolStatus]);

  const onFiles = (files: FileList | null) => {
    const nextPaths = pickPaths(files);
    setPaths(nextPaths);
    setSummary(null);
    setFeedback(null);
    setJobState(nextPaths.length > 0 ? "ready" : "idle");
    setProgress(nextPaths.length > 0 ? 8 : 0);
    if (nextPaths.length > 0) {
      onNotify({ level: "info", message: `已选择 ${nextPaths.length} 个文件` });
    }
  };

  const onPickFiles = async () => {
    try {
      const selectedPaths = await pickWechatFiles();
      setPaths(selectedPaths);
      setSummary(null);
      setFeedback(null);
      setJobState(selectedPaths.length > 0 ? "ready" : "idle");
      setProgress(selectedPaths.length > 0 ? 8 : 0);
      if (selectedPaths.length > 0) {
        onNotify({ level: "info", message: `已选择 ${selectedPaths.length} 个文件` });
      }
    } catch (e) {
      const message = toUserMessage(e, "文件选择失败");
      setFeedback({
        source: "import",
        level: "error",
        message,
        nextStep: "下一步：检查文件权限后重试；若仍失败，请在 Finder 中确认微信导出目录可访问。",
      });
      onNotify({ level: "error", message });
    }
  };

  const onCheckTools = async () => {
    try {
      const status = await getLocalToolStatus();
      setToolStatus(status);
      const missing = missingTools(status);
      if (missing.length > 0) {
        onNotify({
          level: "warning",
          message: `工具检查结果：缺少 ${missing.join("、")}，请先补齐再导入。`,
        });
      } else {
        onNotify({ level: "success", message: "工具检查结果：本地导入能力可用。" });
      }
    } catch (e) {
      onNotify({ level: "error", message: toUserMessage(e, "工具检查失败") });
    }
  };

  const onImport = async () => {
    setLoading(true);
    setFeedback(null);
    setJobState("importing");
    setProgress((prev) => Math.max(prev, 12));
    try {
      const toolStatus = await getLocalToolStatus();
      setToolStatus(toolStatus);
      const missing = missingTools(toolStatus);
      if (missing.length > 0) {
        const message = `检测到缺少本地导入能力：${missing.join("、")}。涉及 PDF/文档导出文件暂不可解析，请补齐后再导入。`;
        setFeedback({
          source: "import",
          level: "warning",
          message,
          nextStep: "下一步：点击“检查工具”确认环境，或去“设置与日志”检查配置。",
        });
        setJobState("failed");
        onNotify({ level: "warning", message });
        return;
      }

      const result = await importWechatAndPersist(paths);
      setSummary(result);
      setProgress(100);
      setHistory((prev) => {
        const nextItem: ImportHistoryItem = {
          id: result.batch_id,
          fileCount: paths.length,
          status: result.parse_failed + result.persist_failed > 0 ? "partial" : "success",
          summary: result,
          createdAt: new Date().toISOString(),
        };
        return [nextItem, ...prev].slice(0, 20);
      });
      const failed = result.parse_failed + result.persist_failed;
      const importOutcome: ImportJobState =
        failed === 0
          ? "success"
          : result.persisted > 0 || result.parsed_ok > 0
            ? "partial"
            : "failed";
      setJobState(importOutcome);
      setFeedback(
        importOutcome === "success"
          ? {
              source: "import",
              level: "success",
              message: `导入状态：成功。解析成功 ${result.parsed_ok}，入库 ${result.persisted}。`,
              nextStep: "下一步：进入审核队列，处理新增内容。",
            }
          : importOutcome === "partial"
            ? {
                source: "import",
                level: "warning",
                message: `导入状态：部分成功。解析成功 ${result.parsed_ok}，入库 ${result.persisted}，失败 ${failed}。`,
                nextStep: "下一步：点击“前往异常标签页”处理失败项，再继续导入。",
              }
            : {
                source: "import",
                level: "error",
                message: `导入状态：失败。失败 ${failed}，未完成入库。`,
                nextStep: "下一步：点击“重试导入”；若持续失败，请去“设置与日志”检查工具环境。",
              }
      );
      onNotify({
        level:
          importOutcome === "success"
            ? "success"
            : importOutcome === "partial"
              ? "warning"
              : "error",
        message:
          importOutcome === "success"
            ? `导入状态：成功。解析成功 ${result.parsed_ok}，入库 ${result.persisted}。`
            : importOutcome === "partial"
              ? `导入状态：部分成功。解析成功 ${result.parsed_ok}，入库 ${result.persisted}，失败 ${failed}。`
              : `导入状态：失败。失败 ${failed}，请重试或检查设置。`,
      });
    } catch (e) {
      const message = toUserMessage(e, "导入失败");
      setFeedback({
        source: "import",
        level: "error",
        message,
        nextStep: "下一步：点击“重试导入”；若持续失败，请去“设置与日志”检查环境后再试。",
      });
      setJobState("failed");
      onNotify({ level: "error", message });
    } finally {
      setLoading(false);
    }
  };

  const onSyncXhs = async () => {
    setXhsSyncing(true);
    setFeedback(null);
    try {
      const result = await collectSource("xhs");
      const message = `小红书同步完成：抓取 ${result.fetched}，入库 ${result.stored}。`;
      onNotify({ level: "success", message });
      setFeedback({
        source: "xhs",
        level: "success",
        message,
        nextStep: "下一步：进入审核队列查看新增项。",
      });
    } catch (e) {
      const message = toUserMessage(e, "小红书同步失败");
      onNotify({ level: "error", message });
      setFeedback({
        source: "xhs",
        level: "error",
        message,
        nextStep:
          "下一步：确认 Chrome 已登录小红书，并在开发者菜单开启“允许 Apple 事件中的 JavaScript”。",
      });
    } finally {
      setXhsSyncing(false);
    }
  };

  useEffect(() => {
    localStorage.setItem(IMPORT_HISTORY_KEY, JSON.stringify(history));
  }, [history]);

  useEffect(() => {
    if (!loading) {
      return;
    }
    const timer = window.setInterval(() => {
      setProgress((prev) => {
        if (prev >= 88) {
          return prev;
        }
        return Math.min(prev + 9, 88);
      });
    }, 350);
    return () => window.clearInterval(timer);
  }, [loading]);

  const jobStateText = (() => {
    if (jobState === "idle") {
      return "准备中";
    }
    if (jobState === "ready") {
      return "待开始";
    }
    if (jobState === "importing") {
      return "导入中";
    }
    if (jobState === "success") {
      return "已完成";
    }
    if (jobState === "partial") {
      return "部分成功";
    }
    return "失败";
  })();

  const jobStateHint = (() => {
    if (jobState === "importing") {
      return "正在导入与去重，请稍候...";
    }
    if (jobState === "success") {
      return "导入成功，可继续进入审核队列。";
    }
    if (jobState === "partial") {
      return "存在失败项，建议处理异常后再继续导入。";
    }
    if (jobState === "failed") {
      return "导入失败，可重试或检查设置与工具环境。";
    }
    if (jobState === "ready") {
      return "文件已就绪，可开始导入。";
    }
    return "准备中";
  })();

  return (
    <section>
      <header className="page-header">
        <h1>导入中心</h1>
        <p>支持微信文件导入，也支持一键同步小红书点赞与收藏。</p>
      </header>

      <div className="inline-actions">
        <button
          type="button"
          className="button-primary"
          onClick={onSyncXhs}
          disabled={xhsSyncing || loading}
          data-testid="import-sync-xhs"
        >
          {xhsSyncing ? "同步中..." : "一键同步小红书"}
        </button>
        <span className="hint">同步范围：点赞 + 收藏（增量遇已入库指纹自动停止）</span>
      </div>

      <div
        className="dropzone"
        data-testid="import-dropzone"
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          onFiles(event.dataTransfer.files);
        }}
      >
        <p>拖拽微信导出文件到这里，或点击下方按钮（默认打开微信本地目录）</p>
        <button type="button" onClick={onPickFiles} data-testid="import-pick-files">
          选择微信导出文件
        </button>
      </div>

      <div className="inline-list">
        <strong>待导入文件</strong>
        {paths.length === 0 ? <p>尚未选择文件</p> : <p>{paths.join(" · ")}</p>}
      </div>

      {activeMissingTools.length > 0 && (
        <div
          className="state-panel error"
          role="alert"
          aria-live="assertive"
          data-testid="import-tools-missing"
        >
          启动检测发现本地导入能力缺失：{activeMissingTools.join("、")}。涉及
          PDF/文档导出文件暂不可解析，请补齐后再导入。
          <div className="inline-actions">
            <button
              type="button"
              onClick={onCheckTools}
              disabled={loading}
              data-testid="import-check-tools"
            >
              检查工具
            </button>
            <button type="button" onClick={onOpenSettings} data-testid="import-open-settings">
              去设置页
            </button>
          </div>
        </div>
      )}

      <article className="job-card">
        <div className="job-head">
          <span>导入任务状态</span>
          <strong className={`job-state ${jobState}`}>{jobStateText}</strong>
        </div>
        <div className="job-track">
          <div className="job-fill" style={{ width: `${progress}%` }} />
        </div>
        <small>{loading ? "正在解析与去重..." : `${jobStateHint}（当前进度 ${progress}%）`}</small>
      </article>

      <div className="inline-actions">
        <button
          type="button"
          className="button-primary"
          onClick={onImport}
          disabled={!canSubmit}
          data-testid="import-start"
        >
          {loading ? "导入中..." : "开始导入"}
        </button>
        <span className="hint">B站采集默认关闭（可在设置页开启）</span>
      </div>

      {feedback && (
        <div
          className={`state-panel ${feedback.level === "error" ? "error" : feedback.level === "warning" ? "warning" : ""}`}
          role={feedback.level === "error" ? "alert" : "status"}
          aria-live={feedback.level === "error" ? "assertive" : "polite"}
          data-testid="import-feedback"
        >
          <div>{feedback.message}</div>
          {feedback.nextStep && <small>{feedback.nextStep}</small>}
          <div className="inline-actions">
            {feedback.source === "import" ? (
              <>
                <button
                  type="button"
                  onClick={onImport}
                  disabled={!canSubmit}
                  data-testid="import-retry"
                >
                  重试导入
                </button>
                <button
                  type="button"
                  onClick={onCheckTools}
                  disabled={loading}
                  data-testid="import-feedback-check-tools"
                >
                  检查工具
                </button>
                <button
                  type="button"
                  onClick={onOpenSettings}
                  data-testid="import-feedback-open-settings"
                >
                  去设置页
                </button>
                {(jobState === "success" || jobState === "partial") && (
                  <button type="button" onClick={onOpenReview} data-testid="import-open-review">
                    前往审核队列
                  </button>
                )}
                {jobState === "partial" && (
                  <button
                    type="button"
                    onClick={onOpenExceptions}
                    data-testid="import-feedback-open-exceptions"
                  >
                    前往异常标签页
                  </button>
                )}
              </>
            ) : (
              <>
                <button
                  type="button"
                  onClick={onSyncXhs}
                  disabled={xhsSyncing || loading}
                  data-testid="xhs-retry"
                >
                  {xhsSyncing ? "重试中..." : "重试小红书同步"}
                </button>
                <button type="button" onClick={onOpenReview} data-testid="xhs-open-review">
                  前往审核队列
                </button>
                <button type="button" onClick={onOpenSettings} data-testid="xhs-open-settings">
                  去设置页
                </button>
              </>
            )}
          </div>
        </div>
      )}

      {summary && (
        <article className="summary-card">
          <h3>导入摘要</h3>
          <div className="summary-grid">
            <div>
              <span>总条数</span>
              <strong>{summary.total}</strong>
            </div>
            <div>
              <span>解析成功</span>
              <strong>{summary.parsed_ok}</strong>
            </div>
            <div>
              <span>重复</span>
              <strong>{summary.duplicates}</strong>
            </div>
            <div>
              <span>解析失败</span>
              <strong>{summary.parse_failed}</strong>
            </div>
            <div>
              <span>入库成功</span>
              <strong>{summary.persisted}</strong>
            </div>
            <div>
              <span>入库失败</span>
              <strong>{summary.persist_failed}</strong>
            </div>
          </div>
          {summary.parse_failed + summary.persist_failed > 0 && (
            <div className="inline-actions">
              <button type="button" onClick={onOpenExceptions} data-testid="import-open-exceptions">
                前往异常标签页
              </button>
              <span className="hint">已自动保留本次摘要，建议先处理异常再继续导入。</span>
            </div>
          )}
        </article>
      )}

      <article className="panel-card">
        <h3>最近导入记录</h3>
        {history.length === 0 ? (
          <p className="hint">暂无导入记录</p>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>时间</th>
                  <th>文件数</th>
                  <th>总条数</th>
                  <th>解析成功</th>
                  <th>入库成功</th>
                  <th>重复</th>
                  <th>总失败</th>
                  <th>状态</th>
                </tr>
              </thead>
              <tbody>
                {history.slice(0, 10).map((item) => (
                  <tr key={item.id}>
                    <td>{new Date(item.createdAt).toLocaleString("zh-CN")}</td>
                    <td>{item.fileCount}</td>
                    <td>{item.summary.total}</td>
                    <td>{item.summary.parsed_ok}</td>
                    <td>{item.summary.persisted}</td>
                    <td>{item.summary.duplicates}</td>
                    <td>{item.summary.parse_failed + item.summary.persist_failed}</td>
                    <td>
                      <span className={`tag ${item.status === "success" ? "success" : "warning"}`}>
                        {item.status === "success" ? "成功" : "部分失败"}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </article>
    </section>
  );
}
