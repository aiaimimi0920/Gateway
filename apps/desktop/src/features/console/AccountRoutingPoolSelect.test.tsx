import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { AccountRoutingPoolSelect } from "./AccountRoutingPoolSelect";

function Harness(props: { initial?: string; disabled?: boolean; onChange: (value: string) => void }) {
  const [groupId, setGroupId] = useState(props.initial ?? "live");
  return <AccountRoutingPoolSelect
    groupId={groupId}
    options={[
      { value: "all", label: "全部分组" },
      { value: "live", label: "Live verified" },
      { value: "pool:", label: "另一个分组" },
    ]}
    label="调整测试账号分组池"
    ungroupedLabel="未分组"
    disabled={props.disabled ?? false}
    onValueChange={(value) => { props.onChange(value); setGroupId(value); }}
  />;
}

describe("AccountRoutingPoolSelect", () => {
  it("renders a themed popup and checked item, not a native option menu", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    const trigger = screen.getByRole("combobox");
    expect(trigger.tagName).toBe("BUTTON");
    expect(trigger).toHaveTextContent("Live verified");
    await user.click(trigger);
    expect(screen.getByRole("listbox")).toHaveClass("nt-account-routing-pool-select__popup");
    expect(screen.getByRole("option", { name: "Live verified" })).toHaveAttribute("data-state", "checked");
    expect(screen.queryByRole("option", { name: "全部分组" })).not.toBeInTheDocument();
    expect(onChange).not.toHaveBeenCalled();
  });

  it("keeps raw group IDs lossless and closes after one selection", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    await user.click(screen.getByRole("combobox"));
    await user.click(screen.getByRole("option", { name: "另一个分组" }));
    expect(onChange).toHaveBeenCalledExactlyOnceWith("pool:");
    expect(screen.getByRole("combobox")).toHaveAttribute("data-group-id", "pool:");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("preserves ungrouping as the original empty ID", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    await user.click(screen.getByRole("combobox"));
    await user.click(screen.getByRole("option", { name: "未分组" }));
    expect(onChange).toHaveBeenCalledExactlyOnceWith("");
    expect(screen.getByRole("combobox")).toHaveTextContent("未分组");
  });

  it("cancels with Escape and restores focus without changing the group", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    const trigger = screen.getByRole("combobox");
    await user.click(trigger);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
    expect(onChange).not.toHaveBeenCalled();
  });

  it("supports arrow-key opening and keyboard selection", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    screen.getByRole("combobox").focus();
    await user.keyboard("{ArrowDown}{End}{Enter}");
    expect(onChange).toHaveBeenCalledExactlyOnceWith("pool:");
  });

  it("does not open or mutate locked and preview accounts", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness disabled onChange={onChange} />);
    expect(screen.getByRole("combobox")).toBeDisabled();
    await user.click(screen.getByRole("combobox"));
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(onChange).not.toHaveBeenCalled();
  });
});
