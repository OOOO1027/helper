import type { ReactNode } from "react";
import type { NavKey, NoticeItem } from "../App";

interface NavItem {
  key: NavKey;
  label: string;
  hint: string;
}

interface Props {
  navItems: NavItem[];
  activeKey: NavKey;
  activeLabel: string;
  onChange: (key: NavKey) => void;
  topBar?: ReactNode;
  children: ReactNode;
  notices: NoticeItem[];
  dismissNotice: (id: string) => void;
}

export function AppShell(props: Props) {
  const { navItems, activeKey, activeLabel, onChange, topBar, children, notices, dismissNotice } =
    props;

  return (
    <div className="app-shell">
      <div className="app-atmosphere" aria-hidden />
      <aside className="sidebar">
        <div className="sidebar-panel">
          <div className="brand">
            <div className="brand-mark" aria-hidden>
              <span className="brand-link" />
              <span className="brand-node node-blue" />
              <span className="brand-node node-amber" />
              <span className="brand-node node-green" />
            </div>
            <div>
              <div className="brand-title">Helper</div>
              <div className="brand-subtitle">抓取 · 审核 · 发布</div>
            </div>
          </div>

          <nav className="nav-list" aria-label="主导航">
            {navItems.map((item) => (
              <button
                key={item.key}
                type="button"
                className={item.key === activeKey ? "nav-item active" : "nav-item"}
                onClick={() => onChange(item.key)}
                data-testid={`nav-${item.key}`}
              >
                <span>{item.label}</span>
                <small>{item.hint}</small>
              </button>
            ))}
          </nav>

          <div className="sidebar-foot">
            <span>v0.2 Desktop</span>
            <small>Notion Final Surface</small>
          </div>
        </div>
      </aside>

      <section className="workspace">
        <header className={topBar ? "top-strip" : "top-strip compact"}>
          <div className="top-context">
            <span>Workspace</span>
            <strong>{activeLabel}</strong>
            <small>One screen, one job</small>
          </div>
          {topBar}
        </header>
        <main className="main-panel">{children}</main>
      </section>

      <div className="notice-stack" role="region" aria-label="应用通知">
        {notices.map((notice) => (
          <div
            className={`notice ${notice.level}`}
            key={notice.id}
            role={notice.level === "error" ? "alert" : "status"}
            aria-live={notice.level === "error" ? "assertive" : "polite"}
          >
            <span>{notice.message}</span>
            <button
              type="button"
              onClick={() => dismissNotice(notice.id)}
              aria-label="关闭通知"
              data-testid="notice-close"
            >
              关闭
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
