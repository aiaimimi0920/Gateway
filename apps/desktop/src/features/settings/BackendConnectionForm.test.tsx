import { useState } from "react";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { BackendConnectionForm } from "./BackendConnectionForm";

it("keeps invalid ports editable and only permits applying a valid changed port", async () => {
  const apply = vi.fn();
  function Form() {
    const [port, setPort] = useState(4400);
    return <BackendConnectionForm current="http://127.0.0.1:4400/ui/" localAddress={`http://127.0.0.1:${port}`} mode="local" remote="" busy="" message="" error={false}
      onMode={vi.fn()} onRemote={vi.fn()} onLocalPort={setPort} onTest={vi.fn()} onApply={apply} />;
  }
  renderWithProviders(<Form />);
  const user = userEvent.setup();
  const port = screen.getByRole("textbox", { name: "本机端口" });
  const button = screen.getByRole("button", { name: "应用" });
  expect(button).toBeDisabled();
  await user.clear(port);
  expect(port).toHaveValue("");
  expect(button).toBeDisabled();
  await user.type(port, "99999");
  expect(port).toHaveValue("99999");
  expect(button).toBeDisabled();
  await user.clear(port);
  await user.type(port, "4500");
  expect(button).toBeEnabled();
  await user.click(button);
  expect(apply).toHaveBeenCalledOnce();
});
