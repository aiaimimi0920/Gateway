import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { ProviderPoolToolbar } from "./ProviderPoolToolbar";
import type { ProviderPoolView } from "./providerPoolView";

function Harness({ scope = "sites" }: { scope?: "sites" | "groups" | "models" }) {
  const [value, onChange] = useState<ProviderPoolView>({ query: "", sort: "default" });
  return <><ProviderPoolToolbar value={value} onChange={onChange} t={(zh) => zh} scope={scope} />
    <output>{JSON.stringify(value.accessKinds)}</output></>;
}
it("supports multiple selections, one selection, no selection and keyboard changes", async () => {
  render(<Harness />);
  const user = userEvent.setup();
  for (const name of ["oauth", "api", "web"]) expect(screen.getByRole("checkbox", { name })).toBeChecked();
  await user.click(screen.getByRole("checkbox", { name: "oauth" }));
  expect(screen.getByRole("status")).toHaveTextContent('["api","web"]');
  await user.click(screen.getByRole("checkbox", { name: "web" }));
  expect(screen.getByRole("status")).toHaveTextContent('["api"]');
  await user.click(screen.getByRole("checkbox", { name: "api" }));
  expect(screen.getByRole("status")).toHaveTextContent('[]');
  await user.keyboard(" ");
  expect(screen.getByRole("status")).toHaveTextContent('["api"]');
});
it.each(["groups", "models"] as const)("does not add site filters to %s", (scope) => {
  render(<Harness scope={scope} />);
  expect(screen.queryByRole("group", { name: "接入方式" })).not.toBeInTheDocument();
});
