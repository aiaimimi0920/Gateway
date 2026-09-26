import { ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";

import type { AccountsLedgerPilotAccount, TranslateFn } from "./accountCardTypes";

export const ACCOUNT_LIBRARY_CARD_WIDTH = 292;
export const ACCOUNT_LIBRARY_GAP = 8;
export const ACCOUNT_LIBRARY_ROWS = 2;

export function AccountLibraryPager(props: {
  t: TranslateFn;
  libraryKey: string;
  accounts: readonly AccountsLedgerPilotAccount[];
  renderAccount: (account: AccountsLedgerPilotAccount) => ReactNode;
}) {
  const { t, libraryKey, accounts, renderAccount } = props;
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [columns, setColumns] = useState(4);
  const [page, setPage] = useState(1);
  const [pageDraft, setPageDraft] = useState("1");
  const pageSize = Math.max(1, columns * ACCOUNT_LIBRARY_ROWS);
  const totalPages = Math.max(1, Math.ceil(accounts.length / pageSize));
  const visibleAccounts = accounts.slice((page - 1) * pageSize, page * pageSize);

  useEffect(() => {
    setPage(1);
    setPageDraft("1");
  }, [libraryKey]);

  useEffect(() => {
    setPage((current) => Math.min(current, totalPages));
  }, [totalPages]);

  useEffect(() => {
    setPageDraft(String(page));
  }, [page]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }
    const updateColumns = (width: number) => {
      if (width <= 0) {
        return;
      }
      setColumns(
        Math.max(
          1,
          Math.floor((width + ACCOUNT_LIBRARY_GAP) / (ACCOUNT_LIBRARY_CARD_WIDTH + ACCOUNT_LIBRARY_GAP)),
        ),
      );
    };
    updateColumns(container.getBoundingClientRect().width);
    if (typeof ResizeObserver === "undefined") {
      return;
    }
    const observer = new ResizeObserver((entries) => updateColumns(entries[0]?.contentRect.width ?? 0));
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  const commitPageDraft = () => {
    const requestedPage = Number.parseInt(pageDraft, 10);
    const nextPage = Number.isFinite(requestedPage)
      ? Math.min(totalPages, Math.max(1, requestedPage))
      : page;
    setPage(nextPage);
    setPageDraft(String(nextPage));
  };

  return (
    <div className="nt-provider-account-library__pager" ref={containerRef}>
      <div
        className="nt-provider-account-card-grid"
        data-account-library-page={page}
        data-account-library-page-size={pageSize}
        data-account-library-rows={ACCOUNT_LIBRARY_ROWS}
        style={{ gridTemplateColumns: `repeat(${columns}, ${ACCOUNT_LIBRARY_CARD_WIDTH}px)` }}
      >
        {visibleAccounts.map(renderAccount)}
      </div>
      {/* The common single-page library does not need pagination chrome. */}
      {totalPages > 1 ? (
        <nav className="nt-provider-account-library__pagination" aria-label={t("账号库分页", "Account library pagination")}>
          <button
            className="nt-icon-action"
            type="button"
            disabled={page <= 1}
            aria-label={t("上一页", "Previous page")}
            onClick={() => setPage((current) => Math.max(1, current - 1))}
          >
            <ChevronLeft size={15} aria-hidden="true" />
          </button>
          <label>
            <span>{t("跳转", "Go to")}</span>
            <input
              className="nt-input nt-provider-account-library__page-input"
              type="number"
              min={1}
              max={totalPages}
              value={pageDraft}
              aria-label={t("跳转页数", "Page number")}
              onChange={(event) => setPageDraft(event.target.value)}
              onBlur={commitPageDraft}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  commitPageDraft();
                }
              }}
            />
          </label>
          <span className="nt-provider-account-library__page-count">/ {totalPages}</span>
          <button
            className="nt-icon-action"
            type="button"
            disabled={page >= totalPages}
            aria-label={t("下一页", "Next page")}
            onClick={() => setPage((current) => Math.min(totalPages, current + 1))}
          >
            <ChevronRight size={15} aria-hidden="true" />
          </button>
        </nav>
      ) : null}
    </div>
  );
}
