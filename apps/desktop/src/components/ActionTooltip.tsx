import * as Tooltip from "@radix-ui/react-tooltip";
import type { ReactElement, ReactNode } from "react";

export type NeuroTooltipProviderProps = {
  children: ReactNode;
};

export type ActionTooltipProps = {
  label: ReactNode;
  children: ReactElement;
};

export function NeuroTooltipProvider({ children }: NeuroTooltipProviderProps) {
  return (
    <Tooltip.Provider delayDuration={350} skipDelayDuration={120}>
      {children}
    </Tooltip.Provider>
  );
}

export function ActionTooltip({ label, children }: ActionTooltipProps) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>{children}</Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          className="nt-tooltip"
          collisionPadding={8}
          side="top"
          sideOffset={6}
        >
          {label}
          <Tooltip.Arrow className="nt-tooltip__arrow" />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}
