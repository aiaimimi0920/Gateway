import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
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
it("confirms layering with an empty optional prompt, exact uni-1 and selected layer resolution",async()=>{
  const user=userEvent.setup(),p=props();render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"layering");
  expect(screen.getByRole("combobox",{name:"模型"})).toHaveValue("uni-1");
  expect(screen.queryByRole("combobox",{name:"时长"})).not.toBeInTheDocument();
  expect(screen.getByRole("textbox",{name:"分层指引（可选）"})).toHaveValue("");
  expect(screen.getByRole("button",{name:"运行测试"})).toBeDisabled();
  await user.type(screen.getByRole("textbox",{name:"源图片引用"}),"https://images.example.test/source.png");
  await user.selectOptions(screen.getByRole("combobox",{name:"图层分辨率"}),"2k");
  await user.click(screen.getByRole("button",{name:"运行测试"}));expect(p.startProviderProbe).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button",{name:"确认生成"}));
  expect(p.startProviderProbe).toHaveBeenCalledWith({prompt:"",model:"uni-1",credentialIds:["exact"],
    media:{operation:"layering",confirmCharge:true,parameters:{source:{url:"https://images.example.test/source.png"},layering:{resolution:"2k"}}}},"luma");
});
it("layering clears old sources, rejects overlong guidance and does not substitute another image model",async()=>{
  const user=userEvent.setup(),p=props();const view=render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"image_edit");
  await user.type(screen.getByRole("textbox",{name:"源图片引用"}),"https://images.example.test/old.png");
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"layering");
  expect(screen.getByRole("textbox",{name:"源图片引用"})).toHaveValue("");
  await user.type(screen.getByRole("textbox",{name:"源图片引用"}),"https://images.example.test/source.png");
  fireEvent.change(screen.getByRole("textbox",{name:"分层指引（可选）"}),{target:{value:"分".repeat(501)}});
  expect(screen.getByRole("button",{name:"运行测试"})).toBeDisabled();
  fireEvent.change(screen.getByRole("textbox",{name:"分层指引（可选）"}),{target:{value:""}});
  expect(screen.getByRole("button",{name:"运行测试"})).toBeEnabled();
  const provider=p.testPolicyDocument!.providers[0] as Record<string,unknown>;
  provider.credentials=[{id:"exact",supported_models:["uni-1-max"]}];
  view.rerender(<PilotActionDialog {...p}/>);
  expect(screen.getByRole("combobox",{name:"模型"})).toHaveValue("");
  expect(screen.getByRole("button",{name:"运行测试"})).toBeDisabled();expect(p.startProviderProbe).not.toHaveBeenCalled();
});
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
it("requires a video source and confirms reframe with its ratio and exact video model",async()=>{
  const user=userEvent.setup(),p=props();render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"video_reframe");
  expect(screen.getByRole("combobox",{name:"模型"})).toHaveValue("ray-3.2");
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"扩展为宽屏");
  expect(screen.getByRole("button",{name:"运行测试"})).toBeDisabled();
  await user.type(screen.getByRole("textbox",{name:"源视频引用"}),"https://media.example.test/source.mp4");
  await user.selectOptions(screen.getByRole("combobox",{name:"目标比例"}),"21:9");
  await user.click(screen.getByRole("button",{name:"运行测试"}));
  expect(p.startProviderProbe).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button",{name:"确认生成"}));
  expect(p.startProviderProbe).toHaveBeenCalledWith({prompt:"扩展为宽屏",model:"ray-3.2",credentialIds:["exact"],
    media:{operation:"video_reframe",confirmCharge:true,parameters:{source:{url:"https://media.example.test/source.mp4",media_type:"video/mp4"},aspect_ratio:"21:9",video:{duration:"5s",resolution:"360p"}}}},"luma");
});
it("blocks vertical 1080p reframe and discards image source when switching operations",async()=>{
  const user=userEvent.setup(),p=props();render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"image_edit");
  await user.type(screen.getByRole("textbox",{name:"源图片引用"}),"https://images.example.test/private.png");
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"video_reframe");
  expect(screen.getByRole("textbox",{name:"源视频引用"})).toHaveValue("");
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"扩画");
  await user.type(screen.getByRole("textbox",{name:"源视频引用"}),"https://media.example.test/source.mp4");
  await user.selectOptions(screen.getByRole("combobox",{name:"目标比例"}),"9:16");
  await user.selectOptions(screen.getByRole("combobox",{name:"分辨率"}),"1080p");
  expect(screen.getByRole("button",{name:"运行测试"})).toBeDisabled();
  expect(screen.getByText("竖向扩画暂不支持 1080p。")).toBeInTheDocument();
  await user.selectOptions(screen.getByRole("combobox",{name:"分辨率"}),"720p");
  expect(screen.getByRole("button",{name:"运行测试"})).toBeEnabled();
  expect(p.startProviderProbe).not.toHaveBeenCalled();
});
it("confirms an exact-account video edit with automatic conditioning and no ignored size controls",async()=>{
  const user=userEvent.setup(),p=props();render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"video_edit");
  expect(screen.getByRole("combobox",{name:"模型"})).toHaveValue("ray-3.2");
  expect(screen.queryByRole("combobox",{name:"时长"})).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox",{name:"目标比例"})).not.toBeInTheDocument();
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"重绘为胶片风格");
  expect(screen.getByRole("button",{name:"运行测试"})).toBeDisabled();
  await user.type(screen.getByRole("textbox",{name:"源视频引用"}),"https://media.example.test/input.mp4");
  await user.selectOptions(screen.getByRole("combobox",{name:"分辨率"}),"720p");
  await user.click(screen.getByRole("button",{name:"运行测试"}));expect(p.startProviderProbe).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button",{name:"确认生成"}));
  expect(p.startProviderProbe).toHaveBeenCalledWith({prompt:"重绘为胶片风格",model:"ray-3.2",credentialIds:["exact"],
    media:{operation:"video_edit",confirmCharge:true,parameters:{source:{url:"https://media.example.test/input.mp4",media_type:"video/mp4"},video:{resolution:"720p",edit:{auto_controls:true}}}}},"luma");
});
it("uses a manual edit strength and clears previous reframe source on operation changes",async()=>{
  const user=userEvent.setup(),p=props();render(<PilotActionDialog {...p}/>,{wrapper:NeuroProvider});
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"video_reframe");
  await user.type(screen.getByRole("textbox",{name:"源视频引用"}),"https://media.example.test/old.mp4");
  await user.selectOptions(screen.getByRole("combobox",{name:"操作"}),"video_edit");
  expect(screen.getByRole("textbox",{name:"源视频引用"})).toHaveValue("");
  await user.selectOptions(screen.getByRole("combobox",{name:"源视频类型"}),"generation_id");
  await user.type(screen.getByRole("textbox",{name:"源视频引用"}),"d290f1ee-6c54-4b01-90e6-d701748f0851");
  await user.selectOptions(screen.getByRole("combobox",{name:"重绘强度"}),"flex_2");
  await user.type(screen.getByRole("textbox",{name:"提示词"}),"风格重绘");
  await user.click(screen.getByRole("button",{name:"运行测试"}));await user.click(screen.getByRole("button",{name:"确认生成"}));
  expect(p.startProviderProbe).toHaveBeenCalledWith(expect.objectContaining({media:{operation:"video_edit",confirmCharge:true,
    parameters:{source:{generation_id:"d290f1ee-6c54-4b01-90e6-d701748f0851"},video:{resolution:"360p",edit:{strength:"flex_2"}}}}}),"luma");
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
it("keeps video MIME on URL and inline sources without changing image or ID projection",()=>{
  const url="https://media.example.test/source.mp4",id="d290f1ee-6c54-4b01-90e6-d701748f0851";
  expect(mediaProbeSource({kind:"url",value:url,mime:"video/mp4"},"video")).toEqual({url,media_type:"video/mp4"});
  expect(mediaProbeSource({kind:"data",value:"YQ==",mime:"video/webm"},"video")).toEqual({data:"YQ==",media_type:"video/webm"});
  expect(mediaProbeSource({kind:"generation_id",value:id,mime:"video/mp4"},"video")).toEqual({generation_id:id});
  expect(mediaProbeSource({kind:"file_id",value:id,mime:"video/mp4"},"video")).toEqual({file_id:id});
  for(const mime of ["image/png","video/*","video/","video/mp4\r\nsecret:x"])
    expect(mediaProbeSource({kind:"url",value:url,mime},"video")).toBeNull();
  expect(mediaProbeSource({kind:"data",value:"YQ==",mime:"video/mp4"})).toBeNull();
});
