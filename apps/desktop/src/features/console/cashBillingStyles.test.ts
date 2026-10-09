import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, it } from "vitest";

it("keeps financial details readable despite shared modal and key-table styles", () => {
  const styles = readFileSync(
    resolve(process.cwd(), "src/features/console/cashBilling.css"),
    "utf8",
  );
  expect(styles).toMatch(
    /\.dialog-content\.nt-cash-dialog--wide\s*\{[^}]*width: min\(960px/,
  );
  expect(styles).toMatch(/\.nt-cash-receipt dt\s*\{[^}]*white-space: normal/);
  expect(styles).toMatch(
    /\.nt-cash-ledger \.nt-key-ledger\s*\{[^}]*min-width: 680px/,
  );
  expect(styles).toMatch(/\.nt-cash-ledger\s*\{[^}]*overflow: auto/);
});
