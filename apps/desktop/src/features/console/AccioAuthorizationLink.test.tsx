import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
it.each(["https://www.accio-ai.com/login", "https://zh.accio.com/login"])("allows the exact Accio login surface %s", (value) => {
  render(<ChatgptAuthorizationLink kind="accio" value={`${value}?state=synthetic`} busy={false} />);
  expect(screen.getByRole("link")).toHaveAttribute("href", `${value}?state=synthetic`);
});
it.each([
  "https://zh.accio.com/login#", "http://zh.accio.com/login", "https://www.accio-ai.com/login/", "https://zh.accio.com/token",
  "https://zh.accio.com.evil.test/login", "https://user:secret@zh.accio.com/login", "https://zh.accio.com:444/login",
  "https://auth.x.ai/oauth2/authorize", "javascript:alert(1)", "https://zh.accio.com/%6cogin",
])("does not execute an unsafe Accio link %s", (value) => {
  render(<ChatgptAuthorizationLink kind="accio" value={value} busy={false} />);
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "打开授权页面" })).toBeDisabled();
});
