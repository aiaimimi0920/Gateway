import { useState } from "react";
import type { ProviderPoolView } from "./providerPoolView";

/** Paginate display results only; routing, search and sorting retain the full catalog. */
export function usePoolPagination<T>(items: readonly T[], view: ProviderPoolView) {
  const pageSize = 24;
  const [selection, setSelection] = useState({ ...view, page: 1 });
  const totalPages = Math.max(1, Math.ceil(items.length / pageSize));
  const viewChanged = selection.query !== view.query || selection.sort !== view.sort;
  const page = viewChanged ? 1 : Math.min(selection.page, totalPages);
  // Adjust before committing children, avoiding a stale page after search/deletion.
  if (viewChanged || selection.page !== page) setSelection({ ...view, page });
  return {
    items: items.slice((page - 1) * pageSize, page * pageSize),
    page,
    totalPages,
    onPage: (next: number) => setSelection({ ...view, page: Math.max(1, Math.min(next, totalPages)) }),
  };
}
