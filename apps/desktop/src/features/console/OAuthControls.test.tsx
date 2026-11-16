import { NeuroProvider } from "@neuro/ui";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { AccioOAuthFields, emptyAccioOAuthFields } from "./AccioOAuthFields";
import { ChatgptOAuthPanel } from "./ChatgptOAuthPanel";
import { ChatgptWebMaterialField } from "./ChatgptWebMaterialField";
import { GoogleWebJsonField } from "./GoogleWebJsonField";
import { GrokOAuthFields, emptyGrokOAuthFields } from "./GrokOAuthFields";

const translate = (zh: string) => zh;
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: translate }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/chatgpt-auth", () => ({ createChatgptAuthApi: () => api }));
const clipboardDescriptor = Object.getOwnPropertyDescriptor(navigator, "clipboard");
afterEach(() => {
  vi.restoreAllMocks();
  if (clipboardDescriptor) Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
  else Reflect.deleteProperty(navigator, "clipboard");
});

function Fields({ disabled = false }: { disabled?: boolean }) {
  const [grok, setGrok] = useState(emptyGrokOAuthFields);
  const [accio, setAccio] = useState(emptyAccioOAuthFields);
  return <NeuroProvider>
    <GrokOAuthFields value={grok} disabled={disabled} onChange={setGrok} />
    <AccioOAuthFields value={accio} editing={false} disabled={disabled} onChange={setAccio} />
  </NeuroProvider>;
}

it("associates every field uniquely and preserves its DOM and focus during edits", () => {
  render(<Fields />);
  const input = screen.getByLabelText("OAuth Client ID");
  input.focus();
  fireEvent.change(input, { target: { value: "permitted-client" } });
  expect(screen.getByLabelText("OAuth Client ID")).toBe(input);
  expect(input).toHaveFocus();
  expect(input).toHaveValue("permitted-client");
  const labels = [...document.querySelectorAll<HTMLLabelElement>(".n-field > label")];
  expect(labels).toHaveLength(11);
  expect(new Set(labels.map(label => label.htmlFor)).size).toBe(labels.length);
  for (const label of labels) {
    expect(document.getElementById(label.htmlFor)).toHaveClass("n-input");
  }
});

it("keeps native fieldset disabling and secret-specific limits", () => {
  render(<Fields disabled />);
  for (const input of document.querySelectorAll("input,select")) expect(input).toBeDisabled();
  expect(screen.getByLabelText("Scopes（空格分隔）")).toHaveAttribute("maxlength", "2048");
  expect(screen.getByLabelText("utdid")).toHaveAttribute("type", "password");
  expect(screen.getByLabelText("utdid")).toHaveAttribute("maxlength", "1024");
  expect(screen.getByLabelText("回调端口")).toHaveAttribute("inputmode", "numeric");
});

it("uses a visible standard file button, resets the native input and requires explicit import", async () => {
  const imported = vi.fn();
  render(<NeuroProvider><GoogleWebJsonField provider="aistudio-web" disabled={false} onImport={imported} /></NeuroProvider>);
  const input = screen.getByLabelText("选择 JSON 文件") as HTMLInputElement;
  const choose = vi.spyOn(input, "click");
  const button = screen.getByRole("button", { name: "选择 JSON 文件" });
  expect(button).toHaveClass("n-button");
  expect(input).toHaveAttribute("hidden");
  fireEvent.click(button);
  expect(choose).toHaveBeenCalledOnce();
  const source = JSON.stringify({ schemaVersion: 1, provider: "aistudio-web" });
  const file = { size: source.length, text: vi.fn(async () => source) };
  for (let index = 0; index < 2; index += 1) {
    await act(async () => { fireEvent.change(input, { target: { files: [file] } }); });
    expect(input.value).toBe("");
    expect(screen.getByLabelText("网页会话 JSON")).toHaveValue(source);
  }
  expect(file.text).toHaveBeenCalledTimes(2);
  expect(imported).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "导入 JSON" }));
  expect(imported).toHaveBeenCalledExactlyOnceWith(source);
});

it("keeps the ChatGPT file limit and does not expose parser input in errors", async () => {
  const onClear = vi.fn(), onMaterial = vi.fn();
  render(<NeuroProvider><ChatgptWebMaterialField disabled={false} loaded={false} onClear={onClear} onMaterial={onMaterial} /></NeuroProvider>);
  const input = screen.getByLabelText("网页会话 JSON");
  expect(screen.getByRole("button", { name: "网页会话 JSON" })).toHaveClass("n-button");
  const text = vi.fn(async () => "private-invalid-material");
  await act(async () => { fireEvent.change(input, { target: { files: [{ size: 128 * 1024 + 1, text }] } }); });
  expect(text).not.toHaveBeenCalled();
  expect(screen.getByRole("alert")).toHaveTextContent("128 KiB");
  await act(async () => { fireEvent.change(input, { target: { files: [{ size: 24, text }] } }); });
  expect(onMaterial).not.toHaveBeenCalled();
  expect(onClear).toHaveBeenCalledTimes(2);
  expect(screen.getByRole("alert")).not.toHaveTextContent("private-invalid-material");
});

it("reports real copy success or failure without replacing the authorization field", async () => {
  const session = { id: "login", status: "waiting_user", message: "Waiting", authorizationUrl: "https://auth.openai.com/oauth/authorize?state=synthetic", credentialId: null };
  api.create.mockResolvedValue({ session }); api.act.mockResolvedValue({ session: { ...session, status: "cancelled" } });
  const writeText = vi.fn().mockResolvedValueOnce(undefined).mockRejectedValueOnce(Error("clipboard denied"));
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  render(<NeuroProvider><ChatgptOAuthPanel client={{ request: vi.fn() }} managementToken="operator" secretGrant="grant"
    providerId="chatgpt" disabled={false} onRequestSecretAccess={vi.fn()} onSaved={vi.fn()} /></NeuroProvider>);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 ChatGPT 登录" })); });
  const input = screen.getByLabelText("授权链接");
  const copy = screen.getByRole("button", { name: "复制授权链接" });
  for (const state of ["copied", "error"]) {
    await act(async () => { fireEvent.click(copy); });
    expect(copy).toHaveAttribute("data-copy-state", state);
    expect(screen.getByLabelText("授权链接")).toBe(input);
    expect(input).toHaveValue(session.authorizationUrl);
  }
  expect(writeText).toHaveBeenNthCalledWith(1, session.authorizationUrl);
  expect(writeText).toHaveBeenNthCalledWith(2, session.authorizationUrl);
  expect(screen.getByRole("alert")).toHaveTextContent("复制失败，请手动复制链接。");
});
