import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));

it("opens only the fixed Luma MCP authorization surface", () => {
  const url = "https://agents.lumalabs.ai/mcp/oauth/authorize?state=synthetic";
  const view = render(<ChatgptAuthorizationLink kind="luma" value={url} busy={false} />);
  expect(screen.getByRole("link")).toHaveAttribute("href", url);
  for (const value of ["https://auth.lumalabs.ai/oauth2/token", "https://agents.lumalabs.ai/mcp", `${url}#`,
    "http://agents.lumalabs.ai/mcp/oauth/authorize", "https://agents.lumalabs.ai:444/mcp/oauth/authorize",
    "https://agents.lumalabs.ai.evil.invalid/mcp/oauth/authorize", "https://user:secret@agents.lumalabs.ai/mcp/oauth/authorize",
    "https://agents.lumalabs.ai/mcp/oauth/%61uthorize", `https://agents.lumalabs.ai/mcp/oauth/authorize?x=${"x".repeat(8192)}`]) {
    view.rerender(<ChatgptAuthorizationLink kind="luma" value={value} busy={false} />);
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  }
});
