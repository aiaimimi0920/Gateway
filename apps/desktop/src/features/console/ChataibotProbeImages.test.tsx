import { NeuroProvider } from "@neuro/ui";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { ChataibotProbeImages } from "./ChataibotProbeImages";

class DeferredReader {
  static all: DeferredReader[] = [];
  result: string | null = null;
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onabort: (() => void) | null = null;
  constructor() { DeferredReader.all.push(this); }
  readAsDataURL = vi.fn();
  abort = vi.fn(() => this.onabort?.());
}
afterEach(() => { vi.unstubAllGlobals(); DeferredReader.all = []; });
const file = () => new File(["x"], "synthetic.png", { type: "image/png" });

it("aborts owned readers on replacement and ignores a late earlier file", async () => {
  vi.stubGlobal("FileReader", DeferredReader);
  const onChange = vi.fn(), user = userEvent.setup();
  render(<ChataibotProbeImages onChange={onChange} disabled={false} t={zh => zh} />, { wrapper: NeuroProvider });
  await user.upload(screen.getByLabelText("源图片"), file());
  const first = DeferredReader.all[0], late = first.onload!;
  await user.upload(screen.getByLabelText("源图片"), file());
  expect(first.abort).toHaveBeenCalledOnce();
  const second = DeferredReader.all[1];
  await act(async () => { second.result = "data:image/png;base64,eA=="; second.onload!(); });
  await act(async () => { first.result = "data:image/png;base64,bGF0ZQ=="; late(); });
  expect(onChange).toHaveBeenLastCalledWith(["data:image/png;base64,eA=="]);
  expect(onChange).not.toHaveBeenCalledWith(["data:image/png;base64,bGF0ZQ=="]);
});
it("unmount aborts local reads and never publishes encoded data afterwards", async () => {
  vi.stubGlobal("FileReader", DeferredReader);
  const onChange = vi.fn(), user = userEvent.setup();
  const view = render(<ChataibotProbeImages onChange={onChange} disabled={false} t={zh => zh} />, { wrapper: NeuroProvider });
  await user.upload(screen.getByLabelText("源图片"), file());
  const reader = DeferredReader.all[0], late = reader.onload!;
  view.unmount(); expect(reader.abort).toHaveBeenCalledOnce();
  await act(async () => { reader.result = "data:image/png;base64,eA=="; late(); });
  expect(onChange.mock.calls).toEqual([[[]]]);
});
it("rejects combined encoded-image overflow even if selected file metadata fit the raw budget", async () => {
  vi.stubGlobal("FileReader", DeferredReader);
  const onChange = vi.fn(), user = userEvent.setup();
  render(<ChataibotProbeImages onChange={onChange} disabled={false} t={zh => zh} />, { wrapper: NeuroProvider });
  await user.upload(screen.getByLabelText("源图片"), [file(), file()]);
  await act(async () => { for (const reader of DeferredReader.all) { reader.result = `data:image/png;base64,${"A".repeat(100000)}`; reader.onload!(); } });
  expect(screen.getByText("图片编码总量超过测试预算。")).toBeInTheDocument();
  expect(onChange.mock.calls).toEqual([[[]]]);
});
