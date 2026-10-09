import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderModelMappingDialog } from "./ProviderModelMappingDialog";

it("adds and deletes custom mappings inline without a save button or mount-time writes", async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderModelMappingDialog open providerId="p" providerLabel="Pool" locked={false}
    modelOptions={["a"]} upstreamModelOptions={["b"]} initialEntries={[]}
    onOpenChange={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>);
  expect(onSubmit).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "保存映射" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "取消" })).not.toBeInTheDocument();
  const source = screen.getByRole("textbox", { name: "自定义用户模型" });
  const target = screen.getByRole("textbox", { name: "自定义映射模型" });
  expect(source.parentElement).toHaveClass("nt-model-map-inline");
  expect(target.parentElement).toHaveClass("nt-model-map-inline");
  await user.type(source, "custom-source{Enter}");
  expect(screen.getByRole("button", { name: "custom-source" })).toHaveAttribute("aria-pressed", "true");
  await user.type(target, "custom-target");
  await user.click(screen.getByRole("button", { name: "添加映射模型" }));
  expect(onSubmit).toHaveBeenLastCalledWith([{ model: "custom-source", upstreamModel: "custom-target" }]);
  expect(screen.getByRole("dialog")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "删除映射模型" }));
  expect(onSubmit).toHaveBeenLastCalledWith([]);
  expect(screen.queryByRole("checkbox", { name: "custom-target" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "删除用户模型" }));
  expect(screen.queryByRole("button", { name: "custom-source" })).not.toBeInTheDocument();
  expect(onSubmit).toHaveBeenCalledTimes(2);
});

it("deletes only the current source's mappings and preserves another source's targets", async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderModelMappingDialog open providerId="p" providerLabel="Pool" locked={false}
    modelOptions={["a", "other"]} upstreamModelOptions={["b"]}
    initialEntries={[{ model: "a", upstreamModel: "b" }, { model: "other", upstreamModel: "b" }]}
    onOpenChange={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>);
  await user.click(screen.getByRole("button", { name: "删除用户模型" }));
  expect(onSubmit).toHaveBeenLastCalledWith([{ model: "other", upstreamModel: "b" }]);
  expect(screen.getByRole("checkbox", { name: "b" })).toBeChecked();
});

it("rejects invalid or over-limit edits without autosaving and exposes persistence errors", async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderModelMappingDialog open providerId="p" providerLabel="Pool" locked={false}
    modelOptions={["a"]} upstreamModelOptions={["extra"]}
    initialEntries={Array.from({ length: 32 }, (_, i) => ({ model: "a", upstreamModel: `target-${i}` }))}
    saveError="Persistence failed" onOpenChange={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>);
  expect(screen.getByRole("alert")).toHaveTextContent("Persistence failed");
  await user.click(screen.getByRole("checkbox", { name: "extra" }));
  expect(screen.getByRole("alert")).toHaveTextContent("最多关联 32");
  expect(screen.getByRole("checkbox", { name: "extra" })).not.toBeChecked();
  await user.type(screen.getByRole("textbox", { name: "自定义映射模型" }), "bad*");
  await user.click(screen.getByRole("button", { name: "添加映射模型" }));
  expect(screen.getByRole("alert")).toHaveTextContent("不含通配符");
  expect(onSubmit).not.toHaveBeenCalled();
});