// Native Fetch over loopback verifies stream cancellation, not a synthetic reader's semantics.
import test from "node:test";
import assert from "node:assert/strict";
import http from "node:http";
import { once } from "node:events";
import vm from "node:vm";
import { executeLumaOperation } from "../lumalabs-session/browser-operation.mjs";
import { event } from "./fixtures/lumalabs-task-browser.mjs";

for (const mode of ["completed", "timeout"]) {
test(`Luma closes real Fetch SSE on ${mode} with the original status`, {timeout:10000}, async () => {
  let subscribed, sourceClosed = false, posts = 0;
  const server = http.createServer((req,res) => {
    if (req.url.endsWith("/signature")) {
      res.setHeader("content-type","application/json");
      res.end(JSON.stringify({ws_token:"fixture",cdn_url:"https://cdn.example.test",query_params:"fixture=1"}));
    } else if (req.url.includes("/events/stream/")) {
      subscribed = res; res.setHeader("content-type","text/event-stream"); res.flushHeaders();
      res.on("close",()=>{sourceClosed=true;});
    } else if (req.url.endsWith("/actions")) {
      posts++; res.setHeader("content-type","application/json");
      res.end(JSON.stringify({output_artifacts:{image:["output-1"]}}));
      if (mode === "completed") subscribed.write(event("finalizing") + event("completed"));
    } else { res.writeHead(404); res.end(); }
  });
  server.listen(0,"127.0.0.1"); await once(server,"listening");
  const signals=[];
  const context=vm.createContext({window:{location:{origin:`http://127.0.0.1:${server.address().port}`}},
    crypto:{randomUUID:()=>"fixture"},AbortController,TextDecoder,setTimeout,clearTimeout,
    fetch:(url,init)=>{signals.push(init.signal);return fetch(url,init);}});
  try {
    const result=await vm.runInContext(`(${executeLumaOperation.toString()})`,context)({
      realmId:"realm-fixture",mediaOperation:"image",artifactField:"image",locale:"zh-CN",
      actionBody:{type:"create_image_uni_1"},autoDiscoverActionType:false,timeoutMs:mode === "completed" ? 3000 : 300,clientCapabilities:"fixture"});
    if (mode === "completed") {
      assert.equal(result.ok,true,JSON.stringify(result)); assert.equal(result.state,"completed");
    } else {
      assert.equal(result.ok,false); assert.equal(result.error.code,"lumalabs_browser_timeout");
      assert.equal(result.error.status,504);
    }
    assert.equal(posts,1); assert.ok(signals.every((signal)=>signal.aborted));
    const deadline=Date.now()+3000;
    while(!sourceClosed && Date.now()<deadline) await new Promise((resolve)=>setTimeout(resolve,20));
    assert.equal(sourceClosed,true);
  } finally {
    const closed=once(server,"close"); server.close(); server.closeAllConnections(); await closed;
  }
});
}
