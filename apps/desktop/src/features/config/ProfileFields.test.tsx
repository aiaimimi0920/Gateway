import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { ProfileFields } from "./ProfileFields";
import type { GatewayProfile } from "../../lib/types";

it("associates the role error in both responsive views without replacing their controls", async () => {
  const profile: GatewayProfile = { name: "Local", runtimeRole: "standalone", port: 4200, gatewayRedisUrl: "", extraEnv: [] };
  const onChange = vi.fn();
  const view = render(<ProfileFields profile={profile} errors={{ runtimeRole: "Invalid role" }} onChange={onChange} />);
  const group = screen.getByRole("radiogroup", { name: "Gateway runtime role" });
  const select = screen.getByRole("combobox", { name: "Gateway runtime role" });
  for (const control of [group, select]) {
    expect(control).toHaveAttribute("aria-invalid", "true");
    expect(control).toHaveAccessibleDescription("Invalid role");
  }
  expect(group.getAttribute("aria-describedby")).not.toBe(select.getAttribute("aria-describedby"));
  await userEvent.selectOptions(select, "worker");
  expect(onChange).toHaveBeenCalledWith({ runtimeRole: "worker" });
  view.rerender(<ProfileFields profile={{ ...profile, runtimeRole: "worker" }} errors={{}} onChange={onChange} />);
  expect(screen.getByRole("combobox", { name: "Gateway runtime role" })).toBe(select);
  expect(select).toHaveFocus();
  expect(select).not.toHaveAttribute("aria-describedby");
  expect(group).not.toHaveAttribute("aria-invalid");
});
