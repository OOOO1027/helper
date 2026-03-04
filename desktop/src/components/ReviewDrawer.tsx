import { useEffect, useRef, useState } from "react";
import type { ReviewItem } from "../types/contracts";

interface Props {
  item: ReviewItem | null;
  busy: boolean;
  onClose: () => void;
  onDecision: (decision: "approved" | "rejected" | "duplicate") => void;
}

function formatState(state: string): string {
  if (state === "pending" || state === "processing") {
    return "待审核";
  }
  if (state === "rejected") {
    return "异常";
  }
  if (state === "done") {
    return "已完成";
  }
  return state;
}

function formatPublishState(state: string): string {
  if (state === "published") {
    return "已发布";
  }
  if (state === "pending") {
    return "待发布";
  }
  if (state === "failed" || state === "retry") {
    return "发布异常";
  }
  if (state === "ignored") {
    return "已忽略";
  }
  return state;
}

function formatQualityBand(band: string): string {
  if (band === "high") {
    return "高";
  }
  if (band === "mid") {
    return "中";
  }
  if (band === "low") {
    return "低";
  }
  return "未知";
}

function uniqueUrls(urls: Array<string | null | undefined>): string[] {
  const normalized = urls
    .map((item) => (typeof item === "string" ? item.trim() : ""))
    .filter((item) => item.length > 0);
  return [...new Set(normalized)];
}

export function ReviewDrawer({ item, busy, onClose, onDecision }: Props) {
  const [copyFeedback, setCopyFeedback] = useState<{
    field: "id" | "normalizedId";
    tone: "success" | "error";
    message: string;
  } | null>(null);
  const copyFeedbackTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (copyFeedbackTimerRef.current !== null) {
        window.clearTimeout(copyFeedbackTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    setCopyFeedback(null);
  }, [item?.id]);

  const setFeedbackWithAutoClear = (
    field: "id" | "normalizedId",
    tone: "success" | "error",
    message: string
  ) => {
    setCopyFeedback({ field, tone, message });
    if (copyFeedbackTimerRef.current !== null) {
      window.clearTimeout(copyFeedbackTimerRef.current);
    }
    copyFeedbackTimerRef.current = window.setTimeout(() => {
      setCopyFeedback(null);
      copyFeedbackTimerRef.current = null;
    }, 1400);
  };

  const copyId = async (value: string, field: "id" | "normalizedId") => {
    try {
      await navigator.clipboard.writeText(value);
      setFeedbackWithAutoClear(field, "success", "已复制");
    } catch {
      setFeedbackWithAutoClear(field, "error", "复制失败，请手动复制");
    }
  };

  const coverUrl = item?.coverUrl?.trim() || null;
  const detailImages = uniqueUrls(item?.imageUrls ?? []);
  const firstThreeImages = detailImages.filter((url) => url !== coverUrl).slice(0, 3);
  const hasPreviewImage = Boolean(coverUrl) || firstThreeImages.length > 0;

  return (
    <aside className={item ? "review-drawer open" : "review-drawer"}>
      {!item && (
        <div className="drawer-empty">
          <p>选择一条记录查看详情</p>
        </div>
      )}

      {item && (
        <>
          <header className="drawer-header">
            <h3>{item.title}</h3>
            <button type="button" className="button-secondary" onClick={onClose}>
              收起
            </button>
          </header>

          <section className="drawer-section">
            {item.url && (
              <div className="kv-row">
                <span>来源链接</span>
                <a href={item.url} target="_blank" rel="noreferrer">
                  打开原文
                </a>
              </div>
            )}
            {!item.url && (
              <div className="kv-row">
                <span>来源链接</span>
                <small className="muted-inline">暂无来源链接</small>
              </div>
            )}
            <div className="kv-row">
              <span>当前状态</span>
              <strong>{formatState(item.state)}</strong>
            </div>
            <div className="kv-row">
              <span>发布状态</span>
              <strong>{formatPublishState(item.publishState)}</strong>
            </div>
            <div className="kv-row">
              <span>优先级</span>
              <strong>{item.priority.toFixed(2)}</strong>
            </div>
            <div className="kv-row">
              <span>内容质量</span>
              <strong>{formatQualityBand(item.qualityBand)}</strong>
            </div>
            <div className="kv-row">
              <span>触发原因</span>
              <strong>{item.reason}</strong>
            </div>
            <section className="preview-section" aria-label="发布预览">
              <h4>发布预览</h4>
              <div className="kv-row">
                <span>结论摘要</span>
                {item.conclusionSummary && item.conclusionSummary.trim().length > 0 ? (
                  <strong>{item.conclusionSummary}</strong>
                ) : (
                  <small className="muted-inline">
                    发生了什么：未生成结论摘要。下一步：在审核备注补充一句可发布结论后再发布。
                  </small>
                )}
              </div>
              <div className="kv-row">
                <span>关键要点</span>
                {item.keyPoints && item.keyPoints.length > 0 ? (
                  <ul className="preview-list">
                    {item.keyPoints.map((point, index) => (
                      <li key={`${point}-${index}`}>{point}</li>
                    ))}
                  </ul>
                ) : (
                  <small className="muted-inline">
                    发生了什么：未生成关键要点。下一步：补充 2-3 条要点后再发布。
                  </small>
                )}
              </div>
              <div className="kv-row">
                <span>标签</span>
                {item.tags && item.tags.length > 0 ? (
                  <div className="preview-tags">
                    {item.tags.map((tag) => (
                      <span key={tag} className="tag neutral">
                        {tag}
                      </span>
                    ))}
                  </div>
                ) : (
                  <small className="muted-inline">
                    发生了什么：未生成标签。下一步：补充至少 1 个检索标签后再发布。
                  </small>
                )}
              </div>
              <div className="kv-row">
                <span>封面与前3图</span>
                {hasPreviewImage ? (
                  <div className="preview-image-grid">
                    {coverUrl && (
                      <figure className="preview-image-cover">
                        <img src={coverUrl} alt="封面预览" loading="lazy" />
                        <figcaption>封面</figcaption>
                      </figure>
                    )}
                    {firstThreeImages.map((url, index) => (
                      <figure key={url} className="preview-image-item">
                        <img src={url} alt={`预览图 ${index + 1}`} loading="lazy" />
                        <figcaption>{`图 ${index + 1}`}</figcaption>
                      </figure>
                    ))}
                  </div>
                ) : (
                  <small className="muted-inline">
                    发生了什么：未检测到封面或图片。下一步：在原文补充封面素材后再发布。
                  </small>
                )}
              </div>
            </section>
            <details className="tech-details">
              <summary>高级信息（排查用）</summary>
              <div className="kv-row">
                <span>ID</span>
                <div className="inline-code-row">
                  <code>{item.id}</code>
                  <button
                    type="button"
                    className="button-secondary"
                    onClick={() => copyId(item.id, "id")}
                    data-testid="review-copy-id"
                  >
                    复制
                  </button>
                  {copyFeedback?.field === "id" && (
                    <small
                      className={`copy-feedback ${copyFeedback.tone}`}
                      role="status"
                      aria-live="polite"
                    >
                      {copyFeedback.message}
                    </small>
                  )}
                </div>
              </div>
              <div className="kv-row">
                <span>归一化ID</span>
                <div className="inline-code-row">
                  <code>{item.normalizedId}</code>
                  <button
                    type="button"
                    className="button-secondary"
                    onClick={() => copyId(item.normalizedId, "normalizedId")}
                    data-testid="review-copy-normalized-id"
                  >
                    复制
                  </button>
                  {copyFeedback?.field === "normalizedId" && (
                    <small
                      className={`copy-feedback ${copyFeedback.tone}`}
                      role="status"
                      aria-live="polite"
                    >
                      {copyFeedback.message}
                    </small>
                  )}
                </div>
              </div>
            </details>
          </section>

          <footer className="drawer-actions">
            <button
              type="button"
              className="button-primary"
              onClick={() => onDecision("approved")}
              disabled={busy}
              data-testid="review-decision-approve"
            >
              通过
            </button>
            <button
              type="button"
              className="button-secondary"
              onClick={() => onDecision("rejected")}
              disabled={busy}
              data-testid="review-decision-reject"
            >
              驳回
            </button>
            <button
              type="button"
              className="button-secondary"
              onClick={() => onDecision("duplicate")}
              disabled={busy}
              data-testid="review-decision-duplicate"
            >
              标记重复
            </button>
          </footer>
        </>
      )}
    </aside>
  );
}
