import { expect, it } from "vitest";
import { siteHealthConfigSchema, applySiteHealth } from "./siteHealth";
import { buildCredentialSecretPatches } from "./credentialDocument";
import { parseRouteDocument } from "./routeDocument";

const document=()=>parseRouteDocument(JSON.stringify({providers:[
  {id:"other"},{id:"site",site_health:{profiles:[
    {id:"plus",name:"Plus",account_group_ids:["local-plus"],policy:{monitor_groups:["remote-plus"],adapter:"sub2api-v2",auth:{kind:"bearer",token:null}}},
    {id:"pro",name:"Pro",account_group_ids:["local-pro"],policy:{}}
  ]}}
],model_routes:[],aliases:{}}));

it("retains profiles, adapter and local/remote groups when a document is saved",()=>{
  const active=document();
  const config=siteHealthConfigSchema.parse((active.providers[1] as Record<string,unknown>).site_health);
  const updated=applySiteHealth(active,"site",config);
  const health=(updated.providers[1] as Record<string,unknown>).site_health;
  expect(health).toEqual(config);
  expect(config.profiles[0].account_group_ids).toEqual(["local-plus"]);
  expect(config.profiles[0].policy.monitor_groups).toEqual(["remote-plus"]);
  expect(config.profiles[0].policy.adapter).toBe("sub2api-v2");
  expect(siteHealthConfigSchema.safeParse({auth:{kind:"bearer",token:"plaintext"}}).success).toBe(false);
});

it("remaps monitor secret patches by provider and profile ID, not array slot",()=>{
  const active=document(); const draft=document();
  draft.providers.reverse();
  const provider=draft.providers[0] as {site_health:{profiles:unknown[]}};
  provider.site_health.profiles.reverse();
  const path="/providers/1/site_health/profiles/0/policy/auth/token";
  const patches=buildCredentialSecretPatches({activeDocument:active,draftDocument:draft,
    activeSecrets:[{path,configured:true,fingerprint:null,preview:null}],
    activeSecretPatches:[{path,operation:"replace",value:"private-draft"}]});
  expect(patches).toEqual([{path:"/providers/0/site_health/profiles/1/policy/auth/token",operation:"replace",value:"private-draft"}]);
  provider.site_health.profiles.pop();
  expect(buildCredentialSecretPatches({activeDocument:active,draftDocument:draft,
    activeSecrets:[{path,configured:true,fingerprint:null,preview:null}]})).toEqual([]);
});
