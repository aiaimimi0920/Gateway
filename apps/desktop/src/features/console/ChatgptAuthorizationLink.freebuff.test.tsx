import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const valid = "https://freebuff.com/login?code=synthetic";
it.each([valid, "https://freebuff.com/", "https://freebuff.com:443/approve?code=synthetic"])("allows exact FreeBuff origin on server-issued approval paths: %s", (value) => {
  render(<ChatgptAuthorizationLink kind="freebuff" value={value} busy={false} />);
  expect(screen.getByRole("link")).toHaveAttribute("href", new URL(value).href);
  expect(screen.getByRole("link")).toHaveAttribute("rel", "noopener noreferrer");
});
it.each([
  "javascript:alert(1)", "file:///tmp/approve", "http://freebuff.com/login", "https://freebuff.com:444/login",
  "https://freebuff.com.evil.test/login", "https://freebuff.com./login", "https://www.freebuff.com/login",
  "https://user:secret@freebuff.com/login", "https://freebuff.com/login#fragment", "https://freebuff.com/login#",
  "https://codebuff.com/login", "https://auth.openai.com/oauth/authorize", `https://freebuff.com/login?code=${"x".repeat(8192)}`,
])("never executes an unsafe or unrelated FreeBuff approval URL: %s", (value) => {
  render(<ChatgptAuthorizationLink kind="freebuff" value={value} busy={false} />);
  expect(screen.queryByRole("link")).not.toBeInTheDocument(); expect(screen.getByRole("button")).toBeDisabled();
});
it("disables launch during mutation and exposes native launcher failure without changing the URL", () => {
  const view = render(<ChatgptAuthorizationLink kind="freebuff" value={valid} busy />);
  expect(screen.queryByRole("link")).not.toBeInTheDocument(); view.rerender(<ChatgptAuthorizationLink kind="freebuff" value={valid} busy={false} />);
  fireEvent(window, new Event("gateway:oauth-browser-failed")); expect(screen.getByRole("alert")).toHaveTextContent("无法打开浏览器");
  expect(screen.getByRole("link")).toHaveAttribute("href", valid);
});
