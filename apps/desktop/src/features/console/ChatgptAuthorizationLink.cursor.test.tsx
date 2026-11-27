import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const valid = `https://www.cursor.com/loginDeepControl?challenge=${"x".repeat(43)}&uuid=aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee&mode=login&redirectTarget=cli`;

it("always exposes the validated Cursor link as a selectable fallback, even on an older native shell", () => {
  render(<ChatgptAuthorizationLink kind="cursor" value={valid} busy={false} />);
  const link = screen.getByRole("link", { name: "打开授权页面" });
  expect(link).toHaveAttribute("href", valid);
  expect(link).toHaveAttribute("target", "_blank");
  expect(link).toHaveAttribute("rel", "noopener noreferrer");
  const fallback = screen.getByRole("textbox", { name: "Cursor 授权链接" });
  expect(fallback).toHaveValue(valid);
  expect(fallback).toHaveAttribute("readonly");
  const select = vi.spyOn(fallback as HTMLInputElement, "select");
  fireEvent.focus(fallback);
  expect(select).toHaveBeenCalledOnce();
  expect(screen.getByText(/使用同一链接/)).toBeInTheDocument();
});

it("shows a native rejection and clears it for a new authorization without losing the fallback", () => {
  const view = render(<ChatgptAuthorizationLink kind="cursor" value={valid} busy={false} />);
  fireEvent(window, new Event("gateway:oauth-browser-failed"));
  expect(screen.getByRole("alert")).toHaveTextContent("无法打开浏览器");
  const next = valid.replace("aaaaaaaa", "bbbbbbbb");
  view.rerender(<ChatgptAuthorizationLink kind="cursor" value={next} busy={false} />);
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "Cursor 授权链接" })).toHaveValue(next);
});

it("never exposes a native-open or copy fallback for an unvalidated Cursor URL", () => {
  render(<ChatgptAuthorizationLink kind="cursor" value={`${valid}&accessToken=synthetic`} busy={false} />);
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "打开授权页面" })).toBeDisabled();
});
