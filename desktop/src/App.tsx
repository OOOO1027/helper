import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AppShell } from "./components/AppShell";
import { StatusStrip } from "./components/StatusStrip";
import { InboxPage } from "./pages/InboxPage";
import { PublishCenterPage } from "./pages/PublishCenterPage";
import { ReviewQueuePage } from "./pages/ReviewQueuePage";
import { SystemHealthPage } from "./pages/SystemHealthPage";
import type { ReviewTab } from "./types/contracts";

export type NavKey = "inbox" | "review" | "publish" | "health";

export interface NoticeItem {
  id: string;
  level: "info" | "success" | "warning" | "error";
  message: string;
}

function useNotices() {
  const [items, setItems] = useState<NoticeItem[]>([]);
  const timerRef = useRef<Record<string, number>>({});

  const remove = useCallback((id: string) => {
    const timer = timerRef.current[id];
    if (timer) {
      window.clearTimeout(timer);
      delete timerRef.current[id];
    }
    setItems((prev) => prev.filter((n) => n.id !== id));
  }, []);

  const push = useCallback(
    (item: Omit<NoticeItem, "id">) => {
      const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
      setItems((prev) => [
        { ...item, id },
        ...prev.filter((n) => n.message !== item.message).slice(0, 3)
      ]);
      const timeoutMs = item.level === "error" ? 8000 : 4800;
      timerRef.current[id] = window.setTimeout(() => {
        remove(id);
      }, timeoutMs);
    },
    [remove]
  );

  useEffect(() => {
    return () => {
      Object.values(timerRef.current).forEach((timer) => window.clearTimeout(timer));
      timerRef.current = {};
    };
  }, []);

  return { items, push, remove };
}

export default function App() {
  const [active, setActive] = useState<NavKey>("inbox");
  const [reviewEntry, setReviewEntry] = useState<{ tab: ReviewTab; token: number }>({
    tab: "pending",
    token: 0
  });
  const notices = useNotices();

  const navItems = useMemo(
    () => [
      { key: "inbox" as const, label: "收件箱", hint: "小红书增量" },
      { key: "review" as const, label: "审核", hint: "低置信决策" },
      { key: "publish" as const, label: "发布中心", hint: "Notion即时发布" },
      { key: "health" as const, label: "系统健康", hint: "状态与回放" }
    ],
    []
  );

  return (
    <AppShell
      navItems={navItems}
      activeKey={active}
      activeLabel={navItems.find((item) => item.key === active)?.label ?? "收件箱"}
      onChange={setActive}
      notices={notices.items}
      dismissNotice={notices.remove}
      topBar={active === "health" ? <StatusStrip /> : null}
    >
      {active === "inbox" && (
        <InboxPage
          onNotify={notices.push}
          onOpenReview={() => {
            setReviewEntry({ tab: "pending", token: Date.now() });
            setActive("review");
          }}
          onOpenPublish={() => setActive("publish")}
        />
      )}
      {active === "review" && (
        <ReviewQueuePage
          onNotify={notices.push}
          forceTab={reviewEntry.tab}
          forceToken={reviewEntry.token}
        />
      )}
      {active === "publish" && (
        <PublishCenterPage onNotify={notices.push} onOpenHealth={() => setActive("health")} />
      )}
      {active === "health" && <SystemHealthPage onNotify={notices.push} />}
    </AppShell>
  );
}
