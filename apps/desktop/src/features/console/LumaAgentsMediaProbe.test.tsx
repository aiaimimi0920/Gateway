import { NeuroProvider } from "@neuro/ui";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { PilotActionDialog } from "./PilotActionDialog";
import { pilotAccount, pilotSection } from "./AccountsLedgerWorkspace.fixtures";
import { mediaProbeSource } from "./LumaMediaProbeSource";

function props(): ComponentProps<typeof PilotActionDialog> {
  const account=pilotAccount({ accountId:"exact",providerId:"luma",displayName:"Synthetic Luma" });
  const section=pilotSection({ providerId:"luma",providerIds:["luma"],providerPreset:"lumalabs-api",directAccounts:[account] });
  return { t:zh=>zh,pilotActionDialog:{kind:"probe",providerId:"luma",account},accountPilotSections:[section],
    testPolicyDocument:{providers:[{id:"luma",preset:"lumalabs-api",base_url:"https://agents.lumalabs.ai/v1",
      supported_models:["uni-1","ray-3.2"],credentials:[{id:"exact",supported_models:["uni-1","ray-3.2"]}]}],model_routes:[],aliases:{}},
    closePilotActionDialog:vi.fn(),credentialProbeBusy:null,activePilotManagedAccount:null,activePilotProbeResult:null,
    draftDirty:false,draftMatchesActiveRevision:true,startPilotProbe:vi.fn(),providerProbeBusy:false,providerProbeError:null,providerProbeResponse:null,
    startProviderProbe:vi.fn().mockResolvedValue(undefined),editorLocked:false,pilotScheduleEnabled:false,setPilotScheduleEnabled:vi.fn(),
    pilotScheduleIntervalMinutes:"60",setPilotScheduleIntervalMinutes:vi.fn(),applyProviderProbeSchedule:vi.fn(),telemetryError:null,
    activePilotStatsView:null,applyPilotProbeSchedule:vi.fn() };
}
it("opens the normal exact-account media form without invoking, saving or scheduling a generation", async()=>{
  const p=props();render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  expect(screen.getByRole("combobox",{name:"操作"})).toBeInTheDocument();
  expect(screen.queryByLabelText("启用自动测试")).not.toBeInTheDocument();
  expect(screen.queryByRole("button",{name:"保存策略"})).not.toBeInTheDocument();
  expect(p.startProviderProbe).not.toHaveBeenCalled();
});
it("requires a fresh charge confirmation for each exact-account generation",async()=>{
  const user=userEvent.setup(),p=props();render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"合成测试");
  await user.click(screen.getByRole("button",{name:"运行测试"}));
  expect(p.startProviderProbe).not.toHaveBeenCalled();
  expect(screen.getByRole("alertdialog")).toHaveTextContent("可能消耗供应商额度");
  await user.click(screen.getByRole("button",{name:"确认生成"}));
  expect(p.startProviderProbe).toHaveBeenCalledTimes(1);
  expect(p.startProviderProbe).toHaveBeenCalledWith({prompt:"合成测试",model:"uni-1",credentialIds:["exact"],
    media:{operation:"image",confirmCharge:true,parameters:{}}},"luma");
  await user.click(screen.getByRole("button",{name:"运行测试"}));
  expect(p.startProviderProbe).toHaveBeenCalledTimes(1);
  await user.click(screen.getByRole("button",{name:"取消"}));
  expect(p.startProviderProbe).toHaveBeenCalledTimes(1);
});
it("selects a real video model and explicit duration/resolution without sending on selection",async()=>{
  const user=userEvent.setup(),p=props();render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"video");
  expect(screen.getByRole("combobox",{name:"模型"})).toHaveValue("ray-3.2");
  await user.selectOptions(screen.getByRole("combobox",{name:"时长"}),"10s");
  await user.selectOptions(screen.getByRole("combobox",{name:"分辨率"}),"720p");
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"合成视频");
  expect(p.startProviderProbe).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button",{name:"运行测试"}));await user.click(screen.getByRole("button",{name:"确认生成"}));
  expect(p.startProviderProbe).toHaveBeenCalledWith(expect.objectContaining({model:"ray-3.2",media:{operation:"video",confirmCharge:true,parameters:{video:{duration:"10s",resolution:"720p"}}}}),"luma");
});
it("requires an edit source and submits only that explicit source",async()=>{
  const user=userEvent.setup(),p=props();render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"image_edit");
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"合成编辑");
  expect(screen.getByRole("button",{name:"运行测试"})).toBeDisabled();
  await user.type(screen.getByRole("textbox",{name:"源图片引用"}),"https://images.example.test/source.png");
  await user.click(screen.getByRole("button",{name:"运行测试"}));await user.click(screen.getByRole("button",{name:"确认生成"}));
  expect(p.startProviderProbe).toHaveBeenCalledWith(expect.objectContaining({media:{operation:"image_edit",confirmCharge:true,parameters:{source:{url:"https://images.example.test/source.png"}}}}),"luma");
});
it("invalidates a pending fee confirmation after the target configuration changes",async()=>{
  const user=userEvent.setup(),p=props();const view=render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"合成");await user.click(screen.getByRole("button",{name:"运行测试"}));
  expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  const next=structuredClone(p.testPolicyDocument!);(next.providers[0] as {base_url:string}).base_url="https://other.example.test";
  view.rerender(<PilotActionDialog {...p} testPolicyDocument={next}/>);
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();expect(screen.getByRole("button",{name:"运行测试"})).toBeDisabled();
  expect(p.startProviderProbe).not.toHaveBeenCalled();
});
it("blocks unsaved drafts, disabled accounts and missing models",async()=>{
  const user=userEvent.setup(),p=props();const view=render(<PilotActionDialog {...p} draftDirty/>,{wrapper:NeuroProvider});
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"合成");expect(screen.getByRole("button",{name:"运行测试"})).toBeDisabled();
  const next=structuredClone(p.testPolicyDocument!);(next.providers[0] as {credentials:{enabled?:boolean}[]}).credentials[0].enabled=false;
  view.rerender(<PilotActionDialog {...p} testPolicyDocument={next}/>);expect(screen.getByRole("button",{name:"运行测试"})).toBeDisabled();
  expect(p.startProviderProbe).not.toHaveBeenCalled();
});
it("keeps default text-plan UI for other products and does not open a paid media form",()=>{
  const p=props();(p.testPolicyDocument!.providers[0] as {preset:string}).preset="lumalabs";
  render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  expect(screen.queryByRole("combobox",{name:"操作"})).not.toBeInTheDocument();expect(p.startProviderProbe).not.toHaveBeenCalled();
});
it("supports bounded URL, generation/file and inline source values without fetching any source",()=>{
  const id="d290f1ee-6c54-4b01-90e6-d701748f0851";
  expect(mediaProbeSource({kind:"generation_id",value:id,mime:"image/png"})).toEqual({generation_id:id});
  expect(mediaProbeSource({kind:"file_id",value:id,mime:"image/png"})).toEqual({file_id:id});
  expect(mediaProbeSource({kind:"data",value:"YQ==",mime:"image/png"})).toEqual({data:"YQ==",media_type:"image/png"});
  for(const value of ["http://x.test/a","https://user:pass@x.test/a","https://x.test/a#secret"])expect(mediaProbeSource({kind:"url",value,mime:"image/png"})).toBeNull();
  expect(mediaProbeSource({kind:"data",value:"x".repeat(180001),mime:"image/png"})).toBeNull();
  expect(mediaProbeSource({kind:"generation_id",value:"../other",mime:"image/png"})).toBeNull();
});
