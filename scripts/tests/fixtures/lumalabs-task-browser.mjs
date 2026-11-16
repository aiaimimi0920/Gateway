// Explicit serialized-page fixture; no browser, upstream, cookies or credential store access.
import vm from "node:vm";
import { executeLumaOperation } from "../../lumalabs-session/browser-operation.mjs";

export async function run({ frames = [], action = {}, actionStatus = 200, streamType = "text/event-stream",
  signaturePadding = "", cancelError = false, cancelKind = "events", invalidMenus = false, overrides = {} } = {}) {
  const calls = [], readers = [], signals = [], timers = new Set();
  let expire;
  function body(parts, kind) {
    let chunks = parts.map((p) => typeof p === "string" ? new TextEncoder().encode(p) : p);
    const reader = { kind, cancelled: 0, released: 0,
      read: async () => chunks.length ? { done: false, value: chunks.shift() } : { done: true },
      cancel: async () => { reader.cancelled++; if (cancelError && kind === cancelKind) throw new Error("private-cancel"); },
      releaseLock: () => { reader.released++; },
    };
    return { getReader: () => { readers.push(reader); return reader; } };
  }
  const response = (value, status, kind) => {
    const text = JSON.stringify(value);
    return { ok: status >= 200 && status < 300, status, headers: new Headers({ "content-type":"application/json" }),
      body: body([text],kind), text: async () => text };
  };
  const fetch = async (url, init) => {
    calls.push({ url, init }); signals.push(init.signal);
    if (url.endsWith("/signature")) return response({ ws_token:"fixture-ws", cdn_url:"https://cdn.example.test",
      query_params:"fixture=1", padding:signaturePadding },200,"signature");
    if (url.includes("/events/stream/")) return { ok:true,status:200,headers:new Headers({"content-type":streamType}),body:body(frames,"events") };
    if (url.includes("/menus.json")) return invalidMenus
      ? { ok:true,status:200,body:body([Uint8Array.from([0xff])],"menus") } : response({},200,"menus");
    if (url.endsWith("/actions")) return response({ output_artifacts:{image:["output-1"]},action:{id:"action-1"},...action },actionStatus,"action");
    throw new Error("Unexpected synthetic URL");
  };
  const context = vm.createContext({ window:{location:{origin:"https://luma.example.test"}},
    crypto:{randomUUID:()=>"fixture"},AbortController,TextDecoder,URL,
    setTimeout:(callback)=>{expire=callback;timers.add(1);return 1;},clearTimeout:(id)=>timers.delete(id),fetch });
  const result = await vm.runInContext(`(${executeLumaOperation.toString()})`,context)({
    realmId:"realm-1",mediaOperation:"image",artifactField:"image",locale:"zh-CN",
    actionBody:{type:"create_image_uni_1"},autoDiscoverActionType:false,timeoutMs:1000,clientCapabilities:"fixture",...overrides });
  return { result:JSON.parse(JSON.stringify(result)),calls,readers,signals,timers,expire };
}
export function event(state, id = "output-1", ref = "right.png") {
  return `data: ${JSON.stringify({artifact:{id,state,object_ref:ref}})}\n\n`;
}
