import { ChevronLeft, ChevronRight } from "lucide-react";

type TranslateFn = (zh: string, en: string) => string;

export type ScopePagerProps = {
  t: TranslateFn;
  page: number;
  totalPages: number;
  navLabel: string;
  onPage: (page: number) => void;
};

/** The credential pool's account-library pager, sized for a card face. */
export function ScopePager({ t, page, totalPages, navLabel, onPage }: ScopePagerProps) {
  if (totalPages <= 1) {
    return null;
  }
  return (
    <nav
      className="nt-provider-account-library__pagination nt-entitlement-scope__pager"
      aria-label={navLabel}
    >
      <button
        className="nt-icon-action"
        type="button"
        title={t("上一页", "Previous page")}
        aria-label={t("上一页", "Previous page")}
        disabled={page <= 1}
        onClick={() => onPage(page - 1)}
      >
        <ChevronLeft size={14} aria-hidden="true" />
      </button>
      <span className="nt-provider-account-library__page-count">
        {page} / {totalPages}
      </span>
      <button
        className="nt-icon-action"
        type="button"
        title={t("下一页", "Next page")}
        aria-label={t("下一页", "Next page")}
        disabled={page >= totalPages}
        onClick={() => onPage(page + 1)}
      >
        <ChevronRight size={14} aria-hidden="true" />
      </button>
    </nav>
  );
}
