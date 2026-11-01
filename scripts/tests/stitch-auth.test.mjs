import assert from "node:assert/strict";
import test from "node:test";
import { bundleFromCookies, captureSession } from "../stitch-auth-worker.mjs";

const cookie = (patch = {}) => ({name:"OSID",value:"synthetic",domain:"stitch.withgoogle.com",path:"/",expires:-1,secure:true,httpOnly:true,sameSite:"Lax",...patch});
test("Stitch captures only cookies that can actually reach its own product origin", () => {
  const bundle = bundleFromCookies([cookie(),cookie({name:"SID",domain:".google.com"}),cookie({name:"ordinary",secure:false}),
    cookie({name:"__Secure-invalid",secure:false}),cookie({name:"partitioned",partitionKey:"https://withgoogle.com"})]);
  assert.equal(bundle.provider,"stitch-web");
  assert.deepEqual(bundle.cookies.map(c=>c.name),["OSID","ordinary"]);
  assert.equal(bundle.cookies[1].secure,false);
  assert.throws(()=>bundleFromCookies([cookie({domain:"evil.withgoogle.com"})]),/stitch_login_session_missing/);
});

test("Stitch waits for the import tab navigation before reading cookies and always closes its browser", async () => {
  let ready=false,closed=0,read=0;
  const page={url:()=>ready?"https://stitch.withgoogle.com/":"about:blank",waitForLoadState:async()=>{}};
  const result=await captureSession({tempRoot:"C:/owned-test"},{chromium:{},executablePath:"fixture",wait:async()=>{ready=true;},
    open:async()=>({browser:{version:()=>"test"},context:{pages:()=>[page],cookies:async urls=>{
      assert.equal(ready,true);assert.deepEqual(urls,["https://stitch.withgoogle.com/"]);read++;return[cookie()];}},close:async()=>{closed++;}})});
  assert.equal(result.provider,"stitch-web");assert.equal(read,1);assert.equal(closed,1);
});

test("Stitch closes the owned browser when imported cookies fail validation", async () => {
  let closed=0;
  await assert.rejects(captureSession({tempRoot:"C:/owned-test"},{chromium:{},executablePath:"fixture",open:async()=>({
    context:{pages:()=>[{url:()=>"https://stitch.withgoogle.com/",waitForLoadState:async()=>{}}],cookies:async()=>[]},
    close:async()=>{closed++;}})}),/stitch_login_session_missing/);
  assert.equal(closed,1);
});
