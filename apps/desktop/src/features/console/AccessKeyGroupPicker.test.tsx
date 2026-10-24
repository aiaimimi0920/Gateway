import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ConsoleAccessKeyGroup } from "../../api/contracts";
import { AccessKeyGroupPicker } from "./AccessKeyGroupPicker";

const groups = [
  { id: "a", name: "基础组", enabled: true, memberCount: 2 },
  { id: "b", name: "高级组", enabled: true, memberCount: 3 },
  { id: "disabled", name: "停用组", enabled: false, memberCount: 1 },
];
function Harness({ rows = groups, initial = ["a"], disabled = false }: { rows?: ConsoleAccessKeyGroup[]; initial?: string[]; disabled?: boolean }) {
  const [selected, setSelected] = useState(initial);
  return <AccessKeyGroupPicker groups={rows} selected={selected} onChange={setSelected} disabled={disabled} t={(zh) => zh} />;
}

describe("entitlement group dropdown", () => {
  it("keeps multi-selection open, supports keyboard navigation and consumes only its own Escape", async () => {
    const escape = vi.fn();
    const user = userEvent.setup();
    render(<div onKeyDown={escape}><Harness /></div>);
    const trigger = screen.getByRole("button", { name: "权益组" });
    expect(trigger).toHaveTextContent("基础组");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    trigger.focus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitemcheckbox", { name: /基础组/ })).toHaveFocus();
    await user.keyboard("{ArrowDown} ");
    expect(screen.getByRole("menuitemcheckbox", { name: /高级组/ })).toBeChecked();
    expect(trigger).toHaveTextContent("基础组、高级组");
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("menuitemcheckbox", { name: /停用组/ })).toBeDisabled();
    escape.mockClear();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
    expect(escape).not.toHaveBeenCalled();
    await user.keyboard("{Escape}");
    expect(escape).toHaveBeenCalled();
  });

  it("searches long lists and allows removing unavailable selections", async () => {
    const user = userEvent.setup();
    render(<Harness rows={[...groups, ...Array.from({ length: 6 }, (_, i) => ({ id: `extra-${i}`, name: `额外组 ${i}`, enabled: true, memberCount: 0 }))]} initial={["missing", "disabled"]} />);
    await user.click(screen.getByRole("button", { name: "权益组" }));
    expect(screen.getByRole("textbox", { name: "搜索权益组" })).toHaveFocus();
    await user.click(screen.getByRole("menuitemcheckbox", { name: /missing.*已移除/ }));
    await user.click(screen.getByRole("menuitemcheckbox", { name: /停用组.*已停用/ }));
    expect(screen.getByRole("button", { name: "权益组" })).toHaveTextContent("选择权益组");
    await user.type(screen.getByRole("textbox"), "额外组 5");
    expect(screen.getAllByRole("menuitemcheckbox")).toHaveLength(1);
    await user.click(screen.getByRole("menuitemcheckbox"));
    expect(screen.getByRole("button", { name: "权益组" })).toHaveTextContent("额外组 5");
    await user.keyboard("{Tab}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes on outside interaction and locks when disabled", async () => {
    const user = userEvent.setup();
    const view = render(<><Harness /><button>外部</button></>);
    await user.click(screen.getByRole("button", { name: "权益组" }));
    await user.click(screen.getByRole("button", { name: "外部" }));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    view.rerender(<Harness disabled />);
    expect(screen.getByRole("button", { name: "权益组" })).toBeDisabled();
  });

  it("keeps the 128-group ceiling while allowing deselection", async () => {
    const user = userEvent.setup();
    const rows = Array.from({ length: 129 }, (_, i) => ({ id: `group-${i}`, name: `权益组 ${i}`, enabled: true, memberCount: 1 }));
    render(<Harness rows={rows} initial={rows.slice(0, 128).map(row => row.id)} />);
    await user.click(screen.getByRole("button", { name: "权益组" }));
    const last = screen.getByRole("menuitemcheckbox", { name: /^权益组 128/ });
    expect(last).toBeDisabled();
    await user.click(screen.getByRole("menuitemcheckbox", { name: /^权益组 0group/ }));
    expect(last).toBeEnabled();
    await user.click(last);
    expect(last).toBeChecked();
    expect(screen.getByRole("menuitemcheckbox", { name: /^权益组 0group/ })).toBeDisabled();
  });
});
