import { expect,it } from "vitest";
import {PROVIDER_CATALOG_TEMPLATES} from "./providerCatalog";
import {addCatalogProvider} from "./providerCatalogCreation";
import {googleWebAuthFamily} from "./GoogleWebAccountImport";
import {defaultCatalogWorkflow} from "./providerCatalogWorkflow";
import {templateAdaptationComplete,templateAdaptationPaused} from "./providerTemplateAdaptation";

it("exposes the verified Stitch design product without aliasing Gemini or enabling paused Flow",()=>{
  const template=PROVIDER_CATALOG_TEMPLATES.find(t=>t.id==="stitch-web")!;
  expect(template).toMatchObject({preset:"stitch-web",baseUrl:"https://stitch.withgoogle.com",supportedModels:[]});
  expect(googleWebAuthFamily(template.preset)).toBe("stitch-web");
  expect(defaultCatalogWorkflow(template)).toMatchObject({specialAdapter:"stitch_web"});
  const document=addCatalogProvider({providers:[],model_routes:[],aliases:{}},template,{
    templateId:template.id,providerId:"custom-stitch-pool",providerLabel:"Stitch",vendorKey:template.vendorKey,
    vendorName:template.vendorName,baseUrl:template.baseUrl,supportedModels:[],
  });
  expect(document.providers[0]).toMatchObject({preset:"stitch-web",explicit_credentials_only:true,credentials:[]});
  expect(document.model_routes).toEqual([]);
  expect(templateAdaptationComplete("stitch-web")).toBe(true);
  expect(templateAdaptationPaused("stitch-web")).toBe(false);
  expect(templateAdaptationPaused("flow-web")).toBe(true);
});
