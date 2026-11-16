import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, it } from "vitest";

it("bounds standard financial tables without retaining the legacy key-table skin", () => {
  const styles = readFileSync(
    resolve(process.cwd(), "src/features/console/cashBilling.css"),
    "utf8",
  );
  expect(styles).toMatch(
    /\.n-dialog\.nt-cash-dialog--wide\s*\{[^}]*width: min\(960px/,
  );
  expect(styles).toMatch(/\.nt-cash-receipt dt\s*\{[^}]*white-space: normal/);
  expect(styles).toMatch(
    /\.nt-cash-ledger \.n-table\s*\{[^}]*min-width: 680px/,
  );
  expect(styles).toMatch(/\.nt-cash-ledger \.n-table-scroll\s*\{[^}]*max-height: 60dvh/);
  expect(styles).not.toContain("nt-key-ledger");
  expect(styles).not.toMatch(/\.nt-cash-receipt summary\s*\{/);
  const dialogStyles = readFileSync(resolve(process.cwd(), "src/features/console/accessKeyCashDialog.css"), "utf8");
  expect(dialogStyles).not.toContain("nt-key-ledger");
  expect(dialogStyles).not.toContain("nt-key-bill-status");
});
