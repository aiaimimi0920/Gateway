import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { FlowmusicAccountImport } from "./FlowmusicAccountImport";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";

it("uses a dedicated FlowMusic login and secret-gated JSON path", () => {
  const onStart = vi.fn(), onImportJson = vi.fn(), onRequestSecretAccess = vi.fn();
  const props = { providerId: "selected-pool", disabled: false, hasSecretAccess: false, onStart, onImportJson, onRequestSecretAccess };
  const view = render(<UiLocaleProvider><FlowmusicAccountImport {...props} /></UiLocaleProvider>);
  fireEvent.click(screen.getByRole("button", { name: "FlowMusic 网页登录并导入" }));
  expect(onStart).toHaveBeenCalledWith("flowmusic-web", "selected-pool", undefined);
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "json" } });
  expect(screen.queryByRole("textbox", { name: "FlowMusic session JSON" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "确认敏感信息访问权限" }));
  expect(onRequestSecretAccess).toHaveBeenCalledOnce();
  view.rerender(<UiLocaleProvider><FlowmusicAccountImport {...props} hasSecretAccess /></UiLocaleProvider>);
  const source = '{"schemaVersion":1,"provider":"flowmusic-web","accessToken":"synthetic.access.token"}';
  fireEvent.change(screen.getByRole("textbox", { name: "FlowMusic session JSON" }), { target: { value: source } });
  fireEvent.click(screen.getByRole("button", { name: "导入 JSON" }));
  expect(onImportJson).toHaveBeenCalledWith("flowmusic-web", "selected-pool", undefined, source);
  expect(screen.getByRole("textbox", { name: "FlowMusic session JSON" })).toHaveValue("");
});
