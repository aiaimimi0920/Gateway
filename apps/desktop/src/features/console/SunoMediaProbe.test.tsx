import { NeuroProvider } from "@neuro/ui";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { PilotActionDialog } from "./PilotActionDialog";
import { sunoProbeProps } from "./SunoMediaProbe.fixtures";

it("opens the normal Suno music form without generating, saving or scheduling", () => {
  const props = sunoProbeProps();
  render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  expect(screen.getByRole("dialog")).toHaveTextContent("音乐测试");
  expect(screen.queryByLabelText("启用自动测试")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "保存策略" })).not.toBeInTheDocument();
  expect(props.startProviderProbe).not.toHaveBeenCalled();
});

it("requires fresh cost consent, sends one exact configured alias, and never schedules it", async () => {
  const props = sunoProbeProps(), user = userEvent.setup();
  render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  await user.selectOptions(screen.getByRole("combobox", { name: "模型" }), "my-song");
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成音乐");
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  expect(props.startProviderProbe).not.toHaveBeenCalled();
  expect(screen.getByRole("alertdialog")).toHaveTextContent("可能消耗生成额度");
  expect(screen.getByRole("alertdialog")).toHaveTextContent("不执行另收费的下载或转换");
  await user.click(screen.getByRole("button", { name: "确认生成" }));
  expect(props.startProviderProbe).toHaveBeenCalledExactlyOnceWith({ prompt: "合成音乐", model: "my-song", credentialIds: ["exact"],
    media: { operation: "music", confirmCharge: true, parameters: {} } }, "suno");
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  await user.click(screen.getByRole("button", { name: "取消" }));
  expect(props.startProviderProbe).toHaveBeenCalledTimes(1);
  expect(props.applyPilotProbeSchedule).not.toHaveBeenCalled();
  expect(props.applyProviderProbeSchedule).not.toHaveBeenCalled();
});

it.each(["dirty", "revision", "disabled", "provider-disabled", "missing", "models", "legacy", "foreign", "mixed-targets", "invalid-mode"])("blocks %s before any cost authorization", async scenario => {
  const props = sunoProbeProps(), user = userEvent.setup();
  const provider = props.testPolicyDocument!.providers[0] as Record<string, unknown>;
  const credentials = provider.credentials as Record<string, unknown>[];
  if (scenario === "dirty") props.draftDirty = true;
  if (scenario === "revision") props.draftMatchesActiveRevision = false;
  if (scenario === "disabled") credentials[0].enabled = false;
  if (scenario === "provider-disabled") provider.enabled = false;
  if (scenario === "missing") provider.credentials = [];
  if (scenario === "models") { provider.supported_models = []; provider.model_map = {}; credentials[0].supported_models = []; }
  if (scenario === "legacy") delete provider.call_flow;
  if (scenario === "foreign") provider.base_url = "https://foreign.invalid";
  if (scenario === "invalid-mode") provider.execution_mode = "unknown-mode";
  if (scenario === "mixed-targets") {
    provider.supported_models = ["my-song"]; credentials[0].supported_models = ["my-song"];
    provider.model_map_targets = { "my-song": ["chirp-v3-5", "not-implemented"] }; provider.model_map = {};
  }
  render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
  expect(props.startProviderProbe).not.toHaveBeenCalled();
});

it.each(["provider", "endpoint", "credential"])("requires fresh consent for bound browser music via %s mode", async scenario => {
  const props = sunoProbeProps(), user = userEvent.setup();
  const provider = props.testPolicyDocument!.providers[0] as Record<string, unknown>;
  if (scenario === "provider") provider.execution_mode = "browser_backed";
  if (scenario === "endpoint") provider.endpoint_execution_modes = { music_generations: "browser_backed" };
  if (scenario === "credential") (provider.credentials as Record<string, unknown>[])[0].execution_mode = "browser_backed";
  render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  await user.selectOptions(screen.getByRole("combobox", { name: "模型" }), "my-song");
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成音乐");
  expect(screen.getByRole("button", { name: "运行测试" })).toBeEnabled();
  expect(props.startProviderProbe).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  await user.click(screen.getByRole("button", { name: "取消" }));
  expect(props.startProviderProbe).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  await user.click(screen.getByRole("button", { name: "确认生成" }));
  expect(props.startProviderProbe).toHaveBeenCalledExactlyOnceWith({ prompt: "合成音乐", model: "my-song", credentialIds: ["exact"],
    media: { operation: "music", confirmCharge: true, parameters: {} } }, "suno");
});

it.each(["callFlow", "credential-auto"])("uses provider flow for %s without inventing a credential override", async scenario => {
  const props = sunoProbeProps(), user = userEvent.setup();
  const provider = props.testPolicyDocument!.providers[0] as Record<string, unknown>;
  if (scenario === "callFlow") { provider.callFlow = provider.call_flow; delete provider.call_flow; }
  else (provider.credentials as Record<string, unknown>[])[0].call_flow = { special_adapter: "auto" };
  render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  expect(screen.getByRole("button", { name: "运行测试" })).toBeEnabled();
  expect(props.startProviderProbe).not.toHaveBeenCalled();
});
