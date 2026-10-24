import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { probePolicySchema } from "./siteHealth";
import { officialHealthAdapters, selectHealthAdapter } from "./officialHealthAdapters";
import { SiteHealthPolicyFields } from "./SiteHealthPolicyFields";
import { SiteHealthAuthFields } from "./SiteHealthAuthFields";
import { SiteHealthReport } from "./SiteHealthReport";
import { useSiteHealthForm } from "./useSiteHealthForm";

const t = (zh: string) => zh;
describe("official public health presets", () => {
  it("sets fixed origins without replacing independent admission settings", () => {
    const before = probePolicySchema.parse({ minimum_success_rate: 88, refresh_interval_secs: 60,
      adapter: "sub2api-v2", auth: { kind: "bearer" }, monitor_groups: ["1"], model_map: { a: "b" }, allow_private_network: true });
    for (const [id, preset] of Object.entries(officialHealthAdapters)) {
      const next = selectHealthAdapter(before, id as keyof typeof officialHealthAdapters);
      expect(probePolicySchema.parse(next).adapter).toBe(id);
      expect(next.site_url).toBe(preset.url);
      expect(next.auth).toEqual({ kind: "none", token: null });
      expect(next.minimum_success_rate).toBe(88);
      expect(next.refresh_interval_secs).toBe(60);
      expect(next.monitor_groups).toEqual([]);
      expect(next.model_map).toEqual({});
      expect(next.allow_private_network).toBe(false);
      expect(selectHealthAdapter(next, "auto").site_url).toBeNull();
    }
  });
  it("actual policy select applies preset in one change", () => {
    const onChange = vi.fn();
    render(<SiteHealthPolicyFields policy={probePolicySchema.parse({})} onChange={onChange} groups="" mapping="" onText={vi.fn()} t={t} />);
    for (const [adapter, preset] of Object.entries(officialHealthAdapters)) {
      fireEvent.change(screen.getByLabelText("适配器"), { target: { value: adapter } });
      expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ adapter, site_url: preset.url, auth: { kind: "none", token: null } }));
    }
  });
  it("hides unnecessary secret controls for fresh public preset", () => {
    render(<SiteHealthAuthFields policy={selectHealthAdapter(probePolicySchema.parse({}), "openai-status")}
      onChange={vi.fn()} secret={{operation:"keep",value:""}} onSecret={vi.fn()} configured={false} hasSecretAccess={false} t={t} />);
    expect(screen.getByText("认证方式：无需认证")).toBeInTheDocument();
    expect(screen.queryByLabelText("监控密钥操作")).not.toBeInTheDocument();
  });
  it("keeps explicit authorized clearing when a stored token exists", () => {
    render(<SiteHealthAuthFields policy={selectHealthAdapter(probePolicySchema.parse({}), "openai-status")}
      onChange={vi.fn()} secret={{operation:"keep",value:""}} onSecret={vi.fn()} configured hasSecretAccess={false} t={t} />);
    expect(screen.getByLabelText("监控密钥操作")).toBeDisabled();
    expect(screen.getByLabelText("监控认证")).toBeDisabled();
  });
  it("reports Codex inheritance without invented percentages", () => {
    render(<SiteHealthReport edited={false} maxAge={900} t={t} report={{provider_id:"p",checked_at:Date.now()/1000,binding:"b",models:{},selected_template:"openai-status",
      probes:[{template:"openai-status",endpoint:"https://status.openai.com/",status:"supported",http_status:200,model_count:0,models:{},observations:[{name:"Codex API",status:"operational",model:null}]}]}} />);
    expect(screen.getByText(/本方案全部模型统一继承/)).toBeInTheDocument();
    expect(screen.queryByText(/100\.0%/)).not.toBeInTheDocument();
  });
  it("explains Qwen announcement coverage before saving and never calls empty notices healthy", () => {
    const policy = render(<SiteHealthPolicyFields policy={selectHealthAdapter(probePolicySchema.parse({}), "qwen-status")}
      onChange={vi.fn()} groups="" mapping="" onText={vi.fn()} t={t} />);
    expect(screen.getByRole("note")).toHaveTextContent("空公告不代表服务正常");
    policy.unmount();
    const view = render(<SiteHealthReport edited={false} maxAge={900} t={t} report={{provider_id:"p",checked_at:Date.now()/1000,binding:"b",models:{},selected_template:"qwen-status",
      probes:[{template:"qwen-status",endpoint:"https://status.aliyun.com/api/status/listEventInProgress",status:"supported",http_status:200,model_count:0,models:{},observations:[
        {name:"Model Studio coverage unconfirmed",status:"no_public_incidents",model:null},
        {name:"<img src=x onerror=alert(1)>",status:"public_notice",model:null}]}]}} />);
    expect(screen.getByText(/未返回进行中的公开公告（不代表服务正常）/)).toBeInTheDocument();
    expect(screen.getByRole("note")).toHaveTextContent("最多显示 32 条公告");
    expect(screen.queryByText(/100\.0%|未报告故障|全部模型统一继承/)).not.toBeInTheDocument();
    expect(view.container.querySelector("img")).toBeNull();
  });
  it("keeps Azure region applicability distinct from health and deployment availability", () => {
    render(<SiteHealthReport edited={false} maxAge={900} t={t} report={{provider_id:"p",checked_at:Date.now()/1000,binding:"b",models:{},selected_template:"azure-status",
      probes:[{template:"azure-status",endpoint:"https://azure.status.microsoft/en-us/status",status:"supported",http_status:200,model_count:0,models:{},observations:[
        {name:"Azure OpenAI / azure-china / China East",status:"not_available",model:null}]}]}} />);
    expect(screen.getByText(/该区域不适用（非故障）/)).toBeInTheDocument();
    expect(screen.getByRole("note")).toHaveTextContent("不代表你的订阅、部署或具体模型可用");
    expect(screen.queryByText(/未报告故障|全部模型统一继承/)).not.toBeInTheDocument();
  });
  it("discards unsaved plaintext without inventing permission to clear stored secrets", () => {
    const provider = {id:"p", base_url:"https://provider.example", site_health:{adapter:"sub2api-v2", auth:{kind:"bearer"}, monitor_groups:["1"]}};
    const hook = renderHook(() => useSiteHealthForm(provider, true));
    act(() => hook.result.current.changeSecret({operation:"replace", value:"private-fixture"}));
    act(() => hook.result.current.changePolicy(selectHealthAdapter(hook.result.current.policy,"openai-status")));
    expect(hook.result.current.secret.value).toBe("");
    expect(hook.result.current.text.groups).toBe("");
    const built = hook.result.current.build();
    expect(built.changes).toEqual([{profileId:null,operation:"keep"}]);
    expect(JSON.stringify(built)).not.toContain("private-fixture");
    expect(built.config?.site_url).toBe("https://status.openai.com/");
  });
});
