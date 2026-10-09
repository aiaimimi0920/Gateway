import { useState } from "react";
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
      <button
        className="nt-icon-action"
        type="button"
        disabled={props.locked}
        aria-label={label}
        title={label}
        onClick={() => setOpen(true)}
      >
        <CircleDollarSign size={14} aria-hidden="true" />
      </button>
      {open ? (
        <AccountBillingDialog {...props} onClose={() => setOpen(false)} />
      ) : null}
    </>
  );
}
