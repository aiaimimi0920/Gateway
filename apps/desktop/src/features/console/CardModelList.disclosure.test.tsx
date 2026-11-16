import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NeuroProvider } from "@neuro/ui";
import { describe, expect, it } from "vitest";
import { CardModelList } from "./CardModelList";

const models = ["openai/gpt-oss-120b", "openai/gpt-oss-20b", "local-custom"];
const t = (_zh: string, en: string) => en;
const renderList = (names = models, total = 10) => <NeuroProvider><CardModelList label="Fixture" t={t} models={names}
  traffic={{ emptyRequestCount: 0, models: new Map([[models[0], { requestCount: total, successCount: total, successWindows: [] }]]) }} /></NeuroProvider>;

describe("standard card model disclosure", () => {
  it("uses independent standard disclosures with linked content and retained row nodes", async () => {
    const { container } = render(renderList());
    const summaries = [...container.querySelectorAll<HTMLDetailsElement>(".n-disclosure > summary")];
    expect(summaries).toHaveLength(2);
    expect(new Set(summaries.map(summary => summary.getAttribute("aria-controls"))).size).toBe(2);
    const first = summaries[0], row = screen.getByText("gpt-oss-120b").closest("li")!;
    const id = first.getAttribute("aria-controls")!;
    expect(document.getElementById(id)).toContainElement(row);
    expect(first).toHaveAttribute("aria-expanded", "false");
    expect(row).not.toBeVisible();
    const user = userEvent.setup();
    await user.click(first);
    expect(first).toHaveAttribute("aria-expanded", "true");
    expect(row).toBeVisible();
    await user.click(summaries[1]);
    expect(first).toHaveAttribute("aria-expanded", "true");
    expect(summaries[1]).toHaveAttribute("aria-expanded", "true");
    await user.click(first);
    expect(first).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText("gpt-oss-120b").closest("li")).toBe(row);
    expect(row).not.toBeVisible();
  });

  it("preserves summary, content, focus and expansion across traffic and catalogue refresh", async () => {
    const { container, rerender } = render(renderList());
    const summary = container.querySelector("summary")!;
    await userEvent.setup().click(summary);
    summary.focus();
    const row = screen.getByText("gpt-oss-120b").closest("li");
    const contentId = summary.getAttribute("aria-controls");
    rerender(renderList([...models, "openai/gpt-oss-120b"], 20));
    expect(container.querySelector("summary")).toBe(summary);
    expect(summary).toHaveFocus();
    expect(summary).toHaveAttribute("aria-expanded", "true");
    expect(summary).toHaveAttribute("aria-controls", contentId);
    expect(summary.querySelector('[data-model-metric="requests"]')).toHaveTextContent("20");
    expect(screen.getByText("gpt-oss-120b").closest("li")).toBe(row);
    expect(container.querySelectorAll('[data-card-model="openai/gpt-oss-120b"]')).toHaveLength(1);
  });

  it("leaves disclosure skin, marker state and keyboard focus to the standard package", () => {
    const component = readFileSync(resolve(process.cwd(), "src/features/console/CardModelList.tsx"), "utf8");
    const css = readFileSync(resolve(process.cwd(), "src/features/console/CardModelList.css"), "utf8");
    expect(component).toContain('import { Disclosure } from "@neuro/ui"');
    expect(component).not.toMatch(/<(details|summary)\b|ChevronRight|onKeyDown/);
    expect(css).not.toMatch(/details\[open\]|summary:hover|details-marker|focus-visible|rotate\(/);
    const summaryRules = [...css.matchAll(/([^{}]+summary[^{}]*)\{([^{}]*)\}/g)];
    expect(summaryRules).toHaveLength(2);
    for (const [, , declarations] of summaryRules) expect(declarations).not.toMatch(/background|color|border|outline|font|padding/);
  });
});
