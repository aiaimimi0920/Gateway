import { useState } from "react";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { validateGatewayProfile } from "../../lib/profileValidation";
import { createProfileFromTemplate } from "../../lib/profileTemplates";
import { createDesktopPanelState } from "../desktopPanels.test-fixtures";
import { ConfigPanel } from "./ConfigPanel";

function Harness() {
  const [profile, setProfile] = useState(() => ({ ...createProfileFromTemplate("local-default"), extraEnv: [{ key: "", value: "one" }] }));
  return <ConfigPanel state={createDesktopPanelState({
    draftProfile: profile, updateDraftProfile: setProfile,
    profileValidation: validateGatewayProfile(profile)
  })} />;
}
describe("standard profile controls", () => {
  it("retains the environment key input and focus across controlled edits", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const input = screen.getByPlaceholderText("KEY");
    await user.type(input, "MY_VARIABLE");
    expect(screen.getByPlaceholderText("KEY")).toBe(input);
    expect(input).toHaveValue("MY_VARIABLE"); expect(input).toHaveFocus();
  });

  it("updates runtime role without changing the other profile fields", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const group = screen.getByRole("radiogroup", { name: "Gateway runtime role" });
    const worker = within(group).getByRole("radio", { name: "worker" });
    const port = screen.getByRole("spinbutton", { name: "PORT" });
    await user.click(worker);
    expect(worker).toBeChecked(); expect(worker).toHaveFocus(); expect(port).toHaveValue(4200);
    expect(screen.getByLabelText("GATEWAY_MANAGEMENT_TOKEN")).toHaveAttribute("type", "password");
    await user.click(screen.getByRole("button", { name: "添加变量" }));
    await user.type(screen.getByLabelText("变量名 2"), "SECOND_KEY");
    await user.type(screen.getByLabelText("值 2"), "two");
    await user.click(screen.getByRole("button", { name: "移除变量 1" }));
    expect(screen.getByLabelText("变量名 1")).toHaveValue("SECOND_KEY");
    expect(screen.getByLabelText("值 1")).toHaveValue("two");
  });

  it("keeps native eligibility, template identity and import/export actions", async () => {
    const user = userEvent.setup(), current = createDesktopPanelState({ importProfileText: "{}" });
    const view = render(<ConfigPanel state={current} />);
    await user.selectOptions(screen.getByRole("combobox", { name: "已保存的 Profile" }), "local-debug");
    expect(current.selectProfile).toHaveBeenCalledWith("local-debug");
    await user.click(screen.getByRole("button", { name: /本地调试/ }));
    expect(current.applyProfileTemplate).toHaveBeenCalledWith("local-debug");
    for (const [label, action] of [["保存 profile", current.saveDraftProfile], ["删除/重置", current.deleteSelectedProfile],
    ["检查路径", current.checkProfilePaths], ["导出脱敏 JSON", current.exportDraftProfile], ["导入到草稿", current.importDraftProfile]] as const) {
      await user.click(screen.getByRole("button", { name: label })); expect(action).toHaveBeenCalledOnce();
    }
    view.rerender(<ConfigPanel state={{ ...current, busy: true }} />);
    for (const label of ["保存 profile", "删除/重置", "检查路径", "导出脱敏 JSON", "导入到草稿"]) {
      expect(screen.getByRole("button", { name: label })).toBeDisabled();
    }
    view.rerender(<ConfigPanel state={{ ...current, isTauriAvailable: false, canSaveProfile: false, importProfileText: " " }} />);
    expect(screen.getByRole("combobox", { name: "已保存的 Profile" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "导入到草稿" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "保存 profile" })).toBeDisabled();
  });

  it("associates real validation errors with unique field instances", () => {
    const current = createDesktopPanelState();
    current.profileValidation.fieldErrors.name = "测试名称不合法";
    render(<><ConfigPanel state={current} /><ConfigPanel state={current} /></>);
    const fields = screen.getAllByLabelText("Profile 名称");
    expect(fields[0].id).not.toBe(fields[1].id);
    for (const input of fields) {
      expect(input).toHaveAttribute("aria-invalid", "true");
      expect(document.getElementById(input.getAttribute("aria-describedby")!)).toHaveTextContent("测试名称不合法");
    }
    expect(screen.getAllByRole("radio", { name: "standalone" })[0].getAttribute("name"))
      .not.toBe(screen.getAllByRole("radio", { name: "standalone" })[1].getAttribute("name"));
  });
});
