import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { kiroAuthorizationUrl } from "./kiroAuthorizationUrl";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
it.each([
  "https://device.sso.us-east-1.amazonaws.com/?user_code=ABCD-EFGH", "https://device.sso.eu-west-1.api.aws/", "https://view.awsapps.com/start/?user_code=ABCD",
])("only opens trusted Kiro device roots and Builder Start paths: %s", (url) => {
  render(<ChatgptAuthorizationLink value={url} kind="kiro" busy={false} />);
  expect(screen.getByRole("link", { name: "打开授权页面" })).toHaveAttribute("href", url);
  expect(screen.getByRole("link")).toHaveAttribute("rel", "noopener noreferrer");
});
it.each([
  "javascript:alert(1)", "http://device.sso.us-east-1.amazonaws.com/", "https://device.sso.us-east-1.amazonaws.com.evil.invalid/",
  "https://evil.invalid/", "https://device.sso.us-east-1.amazonaws.com:8443/", "https://x@device.sso.us-east-1.amazonaws.com/",
  "https://device.sso.us-east-1.amazonaws.com/not-root", "https://device.sso.us-east-1.amazonaws.com/#", "https://view.awsapps.com/",
  "https://organization.awsapps.com/start", " https://view.awsapps.com/start", "https://view.awsapps.com/start?" + "x".repeat(4096),
])("never turns an unsafe challenge into a navigable browser link: %s", (url) => {
  render(<ChatgptAuthorizationLink value={url} kind="kiro" busy={false} />);
  expect(screen.queryByRole("link")).not.toBeInTheDocument(); expect(screen.getByRole("button")).toBeDisabled();
});
it("accepts the fixed Builder ID Start URL without treating arbitrary organization Starts as verification pages", () => {
  expect(kiroAuthorizationUrl("https://view.awsapps.com/start?user_code=ABCD")).toBe("https://view.awsapps.com/start?user_code=ABCD");
});
