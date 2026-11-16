import { NeuroProvider } from "@neuro/ui";
import { render, type RenderOptions } from "@testing-library/react";
import type { ReactNode } from "react";

// Match the application's theme and portal owner, including subsequent rerenders.
export function renderNeuro(ui: ReactNode, options?: Omit<RenderOptions, "wrapper">) {
  return render(ui, { ...options, wrapper: ({ children }) => <NeuroProvider>{children}</NeuroProvider> });
}
