import { PopupSelect } from "@neuro/ui";

import type { AccountCardGroupOption } from "./accountCardTypes";

/** Keep grouping IDs and lossless ungrouping in the application. */
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
  return (
    <PopupSelect
      // Radix disallows an empty item value; prefix every ID so ungrouping stays lossless.
      value={`pool:${props.groupId}`}
      disabled={props.disabled}
      onValueChange={(value) => props.onValueChange(value.slice(5))}
      options={items.map(item => ({ value: `pool:${item.value}`, label: item.label }))}
      displayValue={items.find(item => item.value === props.groupId)?.label ?? props.groupId}
      compact
      className="nt-provider-account-card__group-select"
      aria-label={props.label}
      data-group-id={props.groupId}
    />
  );
}
