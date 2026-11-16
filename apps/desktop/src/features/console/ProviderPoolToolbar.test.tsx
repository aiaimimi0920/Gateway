import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { ProviderPoolToolbar } from "./ProviderPoolToolbar";
import type { ProviderPoolView } from "./providerPoolView";
import { NeuroProvider } from "@neuro/ui";

function Harness({ scope = "sites" }: { scope?: "sites" | "groups" | "models" }) {
  const [value, onChange] = useState<ProviderPoolView>({ query: "", sort: "default" });
  return <NeuroProvider><ProviderPoolToolbar value={value} onChange={onChange} t={(zh) => zh} scope={scope} />
    <output>{JSON.stringify(value)}</output></NeuroProvider>;
}
it("updates search and sort independently and clears only the query", async () => {
  render(<Harness />);
  const user = userEvent.setup();
  await user.type(screen.getByRole("searchbox", { name: "搜索站点" }), "provider");
  await user.click(screen.getByRole("combobox", { name: "站点排序" }));
  await user.click(screen.getByRole("option", { name: "名称降序" }));
  expect(screen.getByRole("status")).toHaveTextContent('{"query":"provider","sort":"name-desc"}');
  await user.click(screen.getByRole("button", { name: "清除站点搜索" }));
  expect(screen.getByRole("status")).toHaveTextContent('{"query":"","sort":"name-desc"}');
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
});
it.each(["groups", "models"] as const)("does not add site filters to %s", (scope) => {
  render(<Harness scope={scope} />);
  expect(screen.queryByRole("group", { name: "接入方式" })).not.toBeInTheDocument();
});
