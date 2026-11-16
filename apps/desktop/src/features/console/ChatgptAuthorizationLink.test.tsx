import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const valid = "https://auth.openai.com/oauth/authorize?state=test&code_challenge=test";

it("allows only the fixed Grok authorization surface", () => {
  const view = render(<ChatgptAuthorizationLink kind="grok" value="https://auth.x.ai/oauth2/authorize?state=test" busy={false} />);
  expect(screen.getByRole("link")).toHaveAttribute("href", "https://auth.x.ai/oauth2/authorize?state=test");
  for (const value of ["https://auth.x.ai/oauth2/token", "https://auth.x.ai.evil.test/oauth2/authorize", "https://auth.x.ai/oauth2/authorize#", "https://auth.openai.com/oauth/authorize", "javascript:alert(1)"]) {
    view.rerender(<ChatgptAuthorizationLink kind="grok" value={value} busy={false} />);
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  }
});

it.each([
  "javascript:alert(1)", "file:///tmp/oauth/authorize", "http://auth.openai.com/oauth/authorize",
  "https://auth.openai.com.evil.example/oauth/authorize", "https://auth.openai.com:444/oauth/authorize",
  "https://user:secret@auth.openai.com/oauth/authorize", "https://auth.openai.com/oauth/token",
  "https://auth.openai.com/oauth/authorize#fragment", "https://auth.openai.com/oauth/authorize#",
  "https://auth.openai.com/oauth/%61uthorize",
  `https://auth.openai.com/oauth/authorize?state=${"x".repeat(8192)}`,
])("rejects unsafe authorization URL %s", (value) => {
  render(<ChatgptAuthorizationLink value={value} busy={false} />);
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "打开授权页面" })).toBeDisabled();
  expect(screen.getByRole("alert")).toHaveTextContent("授权链接无效");
});

it("disables the action while the OAuth session is changing", () => {
  const view = render(<ChatgptAuthorizationLink value={valid} busy />);
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
  expect(screen.getByRole("button")).toBeDisabled();
  view.rerender(<ChatgptAuthorizationLink value={valid} busy={false} />);
  expect(screen.getByRole("link")).toHaveAttribute("href", valid);
});

it("reports native browser failures and removes its listener on unmount", () => {
  const remove = vi.spyOn(window, "removeEventListener");
  const view = render(<ChatgptAuthorizationLink value={valid} busy={false} />);
  fireEvent(window, new Event("gateway:oauth-browser-failed"));
  expect(screen.getByRole("alert")).toHaveTextContent("无法打开浏览器");
  view.unmount();
  expect(remove).toHaveBeenCalledWith("gateway:oauth-browser-failed", expect.any(Function));
  remove.mockRestore();
});
