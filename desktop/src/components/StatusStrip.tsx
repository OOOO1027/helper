import { useEffect, useRef, useState } from "react";
import { getDashboardMetrics } from "../services/ipc";
import { formatPercent } from "../utils/time";

export function StatusStrip() {
  const [todayCollected, setTodayCollected] = useState<number | null>(null);
  const [pendingReview, setPendingReview] = useState<number | null>(null);
  const [budget, setBudget] = useState<number | null>(null);
  const [lastSyncTime, setLastSyncTime] = useState<string>(new Date().toLocaleTimeString("zh-CN"));
  const requestSeqRef = useRef(0);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;

    const load = async () => {
      const requestSeq = requestSeqRef.current + 1;
      requestSeqRef.current = requestSeq;
      try {
        const metrics = await getDashboardMetrics();
        if (!mountedRef.current || requestSeq !== requestSeqRef.current) {
          return;
        }
        setTodayCollected(metrics.today_collected);
        setPendingReview(metrics.pending_review);
        setBudget(metrics.budget_usage_ratio);
        setLastSyncTime(new Date().toLocaleTimeString("zh-CN"));
      } catch {
        if (mountedRef.current && requestSeq === requestSeqRef.current) {
          setTodayCollected(null);
          setPendingReview(null);
          setBudget(null);
        }
      }
    };

    void load();
    const timer = window.setInterval(() => {
      void load();
    }, 60000);

    return () => {
      mountedRef.current = false;
      requestSeqRef.current += 1;
      window.clearInterval(timer);
    };
  }, []);

  return (
    <div className="status-strip">
      <div className="status-item">
        <span>今日采集</span>
        <strong>{todayCollected ?? "-"}</strong>
        <small>当日新增入库</small>
      </div>
      <div className="status-item">
        <span>待审核</span>
        <strong>{pendingReview ?? "-"}</strong>
        <small>队列累计待处理</small>
      </div>
      <div className="status-item">
        <span>预算占用</span>
        <strong>{budget === null ? "-" : formatPercent(budget)}</strong>
        <small>本月模型成本占比</small>
      </div>
      <div className="status-item">
        <span>最近同步</span>
        <strong>{lastSyncTime}</strong>
      </div>
    </div>
  );
}
