import { NeuroProvider } from "@neuro/ui";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { PilotActionDialog } from "./PilotActionDialog";
import { lumalabsProbeProps } from "./LumalabsMediaProbe.fixtures";

it("opens the normal Luma Web form without generating or scheduling", () => {
  const props=lumalabsProbeProps(); render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  expect(screen.getByRole("dialog")).toHaveTextContent("图片任务测试");
  expect(screen.queryByLabelText("启用自动测试")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "保存策略" })).not.toBeInTheDocument();
  expect(props.startProviderProbe).not.toHaveBeenCalled();
});
it("requires fresh charge consent for one configured alias without claiming model health", async () => {
  const props=lumalabsProbeProps(),user=userEvent.setup(); render(<PilotActionDialog {...props} />, { wrapper: NeuroProvider });
  await user.selectOptions(screen.getByRole("combobox", { name: "模型" }), "my-image");
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成任务");
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  expect(props.startProviderProbe).not.toHaveBeenCalled();
  expect(screen.getByRole("alertdialog")).toHaveTextContent("扣额未知");
  expect(screen.getByRole("alertdialog")).toHaveTextContent("模型绑定未验证");
  expect(screen.getByRole("alertdialog")).toHaveTextContent("未恢复模型健康");
  await user.click(screen.getByRole("button", { name: "确认生成" }));
  expect(props.startProviderProbe).toHaveBeenCalledExactlyOnceWith({ prompt:"合成任务", model:"my-image", credentialIds:["exact"],
    media:{operation:"image",confirmCharge:true,parameters:{}} }, "lumalabs");
  await user.click(screen.getByRole("button", { name: "运行测试" })); await user.click(screen.getByRole("button", { name: "取消" }));
  expect(props.startProviderProbe).toHaveBeenCalledTimes(1);
  expect(document.querySelector("audio")).toBeNull();
  expect(props.applyPilotProbeSchedule).not.toHaveBeenCalled();
});
it.each(["dirty","revision","grant","disabled","foreign","legacy","mode","models","mapping","material"])("invalidates pending Lumalabs consent after %s changes",async scenario=>{
  const props=lumalabsProbeProps(),user=userEvent.setup(); const view=render(<PilotActionDialog {...props} />, { wrapper:NeuroProvider });
  await user.type(screen.getByRole("textbox", { name:"提示词" }),"合成"); await user.click(screen.getByRole("button", { name:"运行测试" }));
  expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  const next=structuredClone(props.testPolicyDocument!); const provider=next.providers[0] as Record<string,unknown>;
  if(scenario==="disabled") (provider.credentials as Record<string,unknown>[])[0].enabled=false;
  if(scenario==="foreign") provider.base_url="https://foreign.invalid";
  if(scenario==="legacy") delete provider.call_flow;
  if(scenario==="mode") provider.execution_mode="direct_http";
  if(scenario==="models") { provider.supported_models=[];provider.model_map={};(provider.credentials as Record<string,unknown>[])[0].supported_models=[]; }
  if(scenario==="mapping") provider.model_map={"my-image":"different-native"};
  if(scenario==="material") (provider.credentials as Record<string,unknown>[])[0].api_key="changed-synthetic-session";
  view.rerender(<PilotActionDialog {...props} testPolicyDocument={next} draftDirty={scenario==="dirty"}
    activeRevision={scenario==="revision"?"synthetic-r2":props.activeRevision} authorizationContext={scenario==="grant"?"new-grant":props.authorizationContext}/>);
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(); expect(props.startProviderProbe).not.toHaveBeenCalled();
});

it("shows only the exact account's neutral image facts, never a model-success state", async () => {
  const props=lumalabsProbeProps(),user=userEvent.setup();
  props.startProviderProbe=vi.fn().mockResolvedValue({result:{results:[
    {credentialId:"other",status:"passed",message:"foreign-success"},
    {credentialId:"exact",status:"unsupported",message:"已验证 1 张图片；模型绑定未验证，未恢复模型健康；供应商扣额未知。"},
  ]}});
  render(<PilotActionDialog {...props}/>,{wrapper:NeuroProvider});
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"合成");
  await user.click(screen.getByRole("button",{name:"运行测试"}));
  await user.click(screen.getByRole("button",{name:"确认生成"}));
  expect(screen.getByText(/已验证 1 张图片/)).toBeInTheDocument();
  expect(screen.queryByText("foreign-success")).not.toBeInTheDocument();
  expect(props.applyPilotProbeSchedule).not.toHaveBeenCalled();
});
it("invalidates pending consent after prompt changes and discards a late result on close",async()=>{
  const props=lumalabsProbeProps(),user=userEvent.setup(); let resolve!:(value:unknown)=>void;
  props.startProviderProbe=vi.fn().mockImplementation(()=>new Promise(done=>{resolve=done;}));
  render(<PilotActionDialog {...props}/>,{wrapper:NeuroProvider});
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"合成");await user.click(screen.getByRole("button",{name:"运行测试"}));
  fireEvent.change(screen.getByRole("textbox",{name:"提示词",hidden:true}),{target:{value:"changed"}});
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button",{name:"运行测试"}));await user.click(screen.getByRole("button",{name:"确认生成"}));
  await user.click(screen.getByRole("button",{name:"关闭"}));
  await act(async()=>resolve({result:{results:[{credentialId:"exact",status:"unsupported",message:"late-task-result"}]}}));
  expect(props.closePilotActionDialog).toHaveBeenCalledOnce();expect(screen.queryByText("late-task-result")).not.toBeInTheDocument();
});
