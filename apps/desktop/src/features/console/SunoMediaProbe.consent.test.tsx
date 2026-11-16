import { NeuroProvider } from "@neuro/ui";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { PilotActionDialog } from "./PilotActionDialog";
import { sunoProbeProps } from "./SunoMediaProbe.fixtures";

it.each(["prompt", "mapping", "material", "revision", "dirty", "disabled"])("invalidates pending consent when %s changes", async scenario => {
  const props = sunoProbeProps(), user = userEvent.setup();
  const view = render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  if (scenario === "prompt") fireEvent.change(screen.getByRole("textbox", { name: "提示词", hidden: true }), { target: { value: "changed" } });
  else {
    const document = structuredClone(props.testPolicyDocument!);
    const provider = document.providers[0] as Record<string, unknown>;
    if (scenario === "mapping") provider.model_map = { "my-song": "chirp-auk" };
    if (scenario === "material") (provider.credentials as Record<string, unknown>[])[0].headers = { Cookie: "changed-masked-value" };
    if (scenario === "disabled") (provider.credentials as Record<string, unknown>[])[0].enabled = false;
    view.rerender(<PilotActionDialog {...props} testPolicyDocument={document} draftDirty={scenario === "dirty"} draftMatchesActiveRevision={scenario !== "revision"} />);
  }
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  expect(props.startProviderProbe).not.toHaveBeenCalled();
});

it("cancels local waiting on close and discards a late music result", async () => {
  const props = sunoProbeProps(), user = userEvent.setup(); let resolve!: (value: unknown) => void;
  props.startProviderProbe = vi.fn().mockImplementation(() => new Promise(done => { resolve = done; }));
  render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  await user.click(screen.getByRole("button", { name: "运行测试" })); await user.click(screen.getByRole("button", { name: "确认生成" }));
  await user.click(screen.getByRole("button", { name: "关闭" }));
  expect(props.closePilotActionDialog).toHaveBeenCalledOnce();
  await act(async () => resolve({ result: { results: [{ credentialId: "exact", status: "passed", message: "late-music-success" }] } }));
  expect(screen.queryByText("late-music-success")).not.toBeInTheDocument(); expect(props.startProviderProbe).toHaveBeenCalledOnce();
});

it("renders only the exact account result without a text assessment or audio payload", async () => {
  const props = sunoProbeProps(), user = userEvent.setup();
  props.startProviderProbe = vi.fn().mockResolvedValue({ result: { results: [
    { credentialId: "other", status: "passed", message: "foreign-music-success" },
    { credentialId: "exact", status: "failed", message: "exact-music-failure" }] } });
  render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  await user.click(screen.getByRole("button", { name: "运行测试" })); await user.click(screen.getByRole("button", { name: "确认生成" }));
  expect(await screen.findByText("exact-music-failure")).toBeInTheDocument();
  expect(screen.queryByText("foreign-music-success")).not.toBeInTheDocument();
  expect(document.querySelector("audio")).toBeNull();
});
