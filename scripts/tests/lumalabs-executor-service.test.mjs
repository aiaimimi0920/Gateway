// Actual Node service/CLI over loopback; Playwright is explicitly synthetic, never Chromium.
import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import net from "node:net";
import { fileURLToPath } from "node:url";

async function execute(mode, check, input = {}) {
  const reservation = net.createServer(); reservation.listen(0,"127.0.0.1"); await once(reservation,"listening");
  const port = reservation.address().port; await new Promise((resolve)=>reservation.close(resolve));
  const preload = new URL("./fixtures/lumalabs-worker-lifecycle-preload.mjs",import.meta.url).href;
  const script = fileURLToPath(new URL("../local-browser-executor-service.mjs",import.meta.url));
  const child = spawn(process.execPath,[script],{cwd:fileURLToPath(new URL("../",import.meta.url)),
    env:{...process.env,LOCAL_BROWSER_EXECUTOR_PORT:String(port),GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN:"synthetic-executor-token",
      NODE_OPTIONS:`--import ${preload}`,LUMA_LIFECYCLE_FIXTURE:mode},stdio:["ignore","pipe","pipe"]});
  const exited = once(child,"exit"); let stdout="",stderr="";
  child.stdout.on("data",(chunk)=>{stdout+=chunk;assert.ok(stdout.length<65536);});
  child.stderr.on("data",(chunk)=>{stderr+=chunk;assert.ok(stderr.length<65536);});
  try {
    const base=`http://127.0.0.1:${port}`;
    const deadline=Date.now()+5000;
    while(!stdout.includes('"mode":"local_node_browser_executor_service"')) {
      assert.equal(child.exitCode,null,stderr); assert.ok(Date.now()<deadline,stderr);
      await new Promise((resolve)=>setTimeout(resolve,20));
    }
    const response=await fetch(`${base}/v1/internal/browser-executor/execute`,{method:"POST",signal:AbortSignal.timeout(5000),
      headers:{authorization:"Bearer synthetic-executor-token","content-type":"application/json"},
      body:JSON.stringify({provider:"lumalabs",providerAccountId:"fixture",endpointKind:"images_generations",input:{
        baseUrl:"https://app.lumalabs.ai",realmId:"realm-fixture",mediaOperation:"image",artifactField:"image",
        sessionToken:"synthetic-private",actionBody:{type:"create_image_uni_1"},browserExecutablePath:process.execPath,timeoutMs:2000,...input}})});
    const result=await response.json(); assert.equal(response.status,200); check(result);
  } finally { child.kill("SIGTERM"); await exited; }
}
test("Node remote service preserves the Luma CLI task instead of returning null",{timeout:15000},async()=>{
  await execute("success", (result) => {
    assert.equal(result.ok,true,JSON.stringify(result));
    assert.deepEqual(result.result, {ok:true,signedUrl:"https://asset.fixture.invalid/output.png",
      outputId:"output-1",actionId:null,resolvedActionType:"create_image_uni_1",realmId:"realm-fixture",
      mediaOperation:"image",artifactField:"image",state:"completed"});
  });
});
test("Luma Node service survives early exit while writing stdin",{timeout:15000},async()=>{
  await execute("early-exit",(result)=>assert.equal(result.ok,false),{padding:"x".repeat(512*1024)});
});
test("Luma Node service rejects its input budget before worker dispatch",{timeout:15000},async()=>{
  await execute("success",(result)=>{
    assert.equal(result.ok,false); assert.equal(result.error.code,"lumalabs_browser_executor_input_too_large");
  },{padding:"x".repeat(1024*1024)});
});
test("Luma Node service cannot accept nonzero or oversized worker success",{timeout:30000},async()=>{
  for (const mode of ["nonzero","stdout-budget","stderr-budget"]) {
    await execute(mode, (result) => {
      assert.equal(result.ok,false,mode);
      assert.equal(result.result,null);
      assert.ok(!JSON.stringify(result).includes("synthetic-private"));
    });
  }
});
