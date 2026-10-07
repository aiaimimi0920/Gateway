import { useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import type { AccountsLedgerPilotAccount, TranslateFn } from "./accountCardTypes";

export const ACCOUNT_LIBRARY_CARD_WIDTH = 292;
export const ACCOUNT_LIBRARY_GAP = 8;
export const ACCOUNT_LIBRARY_ROWS = 2;

/** All tabs share one viewport; CSS supplies the same card height even when a tab is empty. */
export function AccountLibraryPager(props: {
  t: TranslateFn;
  libraryKey: string;
  accounts: readonly AccountsLedgerPilotAccount[];
  maxAccountCount?: number;
  renderAccount: (account: AccountsLedgerPilotAccount) => ReactNode;
}) {
  const { t, libraryKey, accounts, maxAccountCount = accounts.length, renderAccount } = props;
  const containerRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const [rows, setRows] = useState(1);
  useLayoutEffect(() => {
    const container = containerRef.current;
    const grid = gridRef.current;
    if (!container || !grid) return;
    const measure = () => {
      // Read used grid tracks, including the stable scrollbar gutter, not the outer library width.
      const tracks = getComputedStyle(grid).gridTemplateColumns;
      const columns = tracks && tracks !== "none"
        ? tracks.trim().split(/\s+/).length
        : Math.max(1, Math.floor((grid.getBoundingClientRect().width + ACCOUNT_LIBRARY_GAP)
          / (ACCOUNT_LIBRARY_CARD_WIDTH + ACCOUNT_LIBRARY_GAP)));
      setRows(Math.max(1, Math.min(ACCOUNT_LIBRARY_ROWS, Math.ceil(maxAccountCount / columns))));
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(container);
    return () => observer.disconnect();
  }, [maxAccountCount]);
  useLayoutEffect(() => {
    if (containerRef.current) containerRef.current.scrollTop = 0;
  }, [libraryKey]);
  return (
    <div className="nt-provider-account-library__pager" ref={containerRef}
      role="region" tabIndex={0} aria-label={t("账号库滚动区域", "Scrollable account library")}
      style={{ "--nt-account-library-visible-rows": rows } as CSSProperties}>
      <div className="nt-provider-account-card-grid" ref={gridRef}
        data-account-library-rows={rows}>
        {accounts.map(renderAccount)}
      </div>
      {accounts.length === 0 ? (
        <div className="nt-provider-account-library__empty">
          <span>{t("当前账号库为空", "Account library is empty")}</span>
        </div>
      ) : null}
    </div>
  );
}
