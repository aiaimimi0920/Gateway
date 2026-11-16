import { expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { act } from "react";
import { buildModelNameIndex, formatModelName, ModelDisplayName, ModelNameProvider } from "./ModelDisplayName";
import { useDisplayPreferences } from "../settings/displayPreferences";

const document = { aliases: { "my/alias": "gpt-4.1" }, model_routes: [], providers: [
  { id: "my-site", label: "My Site", model_map: { renamed: "gpt-4.1" } },
] };
it("formats only presentation names and recognizes configured aliases and mappings", () => {
  const index = buildModelNameIndex(document);
  expect(formatModelName("openai/gpt-4.1", ["my-site"], "off", index)).toBe("gpt-4.1");
  expect(formatModelName("openai/gpt-4.1", ["my-site"], "on", index)).toBe("My Site · gpt-4.1");
  expect(formatModelName("my/alias", ["my-site"], "except-custom", index)).toBe("my/alias");
  expect(formatModelName("renamed", ["my-site"], "except-custom", index)).toBe("renamed");
  expect(formatModelName("renamed", ["my-site"], "on", index)).toBe("My Site · renamed");
  expect(formatModelName("gpt-4.1", ["my-site"], "except-custom", index)).toBe("My Site · gpt-4.1");
  expect(formatModelName("unknown/model", [], "on", index)).toBe("unknown/model");
  expect(document.aliases).toEqual({ "my/alias": "gpt-4.1" });
});
it("updates visible names immediately and persists the chosen display preference", () => {
  localStorage.clear();
  let update!: ReturnType<typeof useDisplayPreferences>["update"];
  function Fixture() { update = useDisplayPreferences().update; return <ModelDisplayName model="gpt-4.1" providerIds={["my-site"]} />; }
  render(<ModelNameProvider document={document}><Fixture /></ModelNameProvider>);
  expect(screen.getByText("gpt-4.1")).toBeVisible();
  act(() => { update({ providerNames: "on" }); });
  expect(screen.getByText("My Site · gpt-4.1")).toBeVisible();
  expect(JSON.parse(localStorage.getItem("gateway-ui-display")!).providerNames).toBe("on");
  localStorage.clear();
});
