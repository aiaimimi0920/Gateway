import * as Select from "@radix-ui/react-select";
import { Check, ChevronDown, ChevronUp } from "lucide-react";

import type { AccountCardGroupOption } from "./accountCardTypes";

/** Own the popup colors instead of relying on Windows native option highlighting. */
export function AccountRoutingPoolSelect(props: {
  groupId: string;
  options: readonly AccountCardGroupOption[];
  label: string;
  ungroupedLabel: string;
  disabled: boolean;
  onValueChange: (groupId: string) => void;
}) {
  const items = [
    { value: "", label: props.ungroupedLabel },
    ...props.options.filter((option) => option.value !== "all" && option.value !== ""),
  ];
  const selectedLabel = items.find((item) => item.value === props.groupId)?.label ?? props.groupId;

  return (
    <Select.Root
      // Radix disallows an empty item value; prefix every ID so ungrouping stays lossless.
      value={`pool:${props.groupId}`}
      disabled={props.disabled}
      onValueChange={(value) => props.onValueChange(value.slice(5))}
    >
      <Select.Trigger
        className="nt-input nt-provider-account-card__group-select"
        aria-label={props.label}
        data-group-id={props.groupId}
      >
        <Select.Value>{selectedLabel}</Select.Value>
        <Select.Icon><ChevronDown size={13} aria-hidden="true" /></Select.Icon>
      </Select.Trigger>
      <Select.Portal>
        <Select.Content
          className="nt-account-routing-pool-select__popup"
          position="popper"
          align="start"
          sideOffset={4}
          collisionPadding={8}
        >
          <Select.ScrollUpButton className="nt-account-routing-pool-select__scroll">
            <ChevronUp size={13} aria-hidden="true" />
          </Select.ScrollUpButton>
          <Select.Viewport className="nt-account-routing-pool-select__viewport">
            {items.map((item) => (
              <Select.Item
                className="nt-account-routing-pool-select__item"
                key={item.value}
                value={`pool:${item.value}`}
                textValue={item.label}
              >
                <Select.ItemText>{item.label}</Select.ItemText>
                <Select.ItemIndicator><Check size={13} aria-hidden="true" /></Select.ItemIndicator>
              </Select.Item>
            ))}
          </Select.Viewport>
          <Select.ScrollDownButton className="nt-account-routing-pool-select__scroll">
            <ChevronDown size={13} aria-hidden="true" />
          </Select.ScrollDownButton>
        </Select.Content>
      </Select.Portal>
    </Select.Root>
  );
}
