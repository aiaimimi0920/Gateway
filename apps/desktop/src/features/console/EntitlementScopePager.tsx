import { ChevronLeft, ChevronRight } from "lucide-react";
import { IconButton } from "@neuro/ui";

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
      <IconButton
        compact
        variant="quiet"
        title={t("上一页", "Previous page")}
        label={t("上一页", "Previous page")}
        disabled={page <= 1}
        onClick={() => onPage(page - 1)}
      >
        <ChevronLeft size={14} aria-hidden="true" />
      </IconButton>
      <span className="nt-entitlement-scope__page-count" style={{ minWidth: `${String(totalPages).length * 2 + 3}ch` }}>
        {page} / {totalPages}
      </span>
      <IconButton
        compact
        variant="quiet"
        title={t("下一页", "Next page")}
        label={t("下一页", "Next page")}
        disabled={page >= totalPages}
        onClick={() => onPage(page + 1)}
      >
        <ChevronRight size={14} aria-hidden="true" />
      </IconButton>
    </nav>
  );
}
