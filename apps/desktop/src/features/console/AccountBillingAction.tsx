import { useState } from "react";
import { IconButton } from "@neuro/ui";
import { CircleDollarSign } from "lucide-react";
import { AccountBillingDialog } from "./AccountBillingDialog";
import type { TranslateFn } from "./accessKeysTypes";

export function AccountBillingAction(props: {
  providerId: string;
  accountId: string;
  name: string;
  locked: boolean;
  t: TranslateFn;
}) {
  const [open, setOpen] = useState(false);
  const label = props.t(
    `账户计费倍率 ${props.name}`,
    `Account billing multiplier ${props.name}`,
  );
  return (
    <>
      <IconButton compact variant="quiet"
        disabled={props.locked}
        label={label}
        title={label}
        onClick={() => setOpen(true)}
      >
        <CircleDollarSign size={14} aria-hidden="true" />
      </IconButton>
      {open ? (
        <AccountBillingDialog {...props} onClose={() => setOpen(false)} />
      ) : null}
    </>
  );
}
