import test from "node:test";
import assert from "node:assert/strict";
import { run, event } from "./fixtures/lumalabs-task-browser.mjs";

test("Luma finalizing and foreign artifacts cannot become a completed task", async () => {
  const {result} = await run({frames:[event("completed","other"),event("finalizing")]});
  assert.equal(result.ok,false);
  assert.equal(result.error.code,"lumalabs_missing_signed_url");
});
test("Luma returns the original action output and exact completed task context", async () => {
  const {result,readers,signals,timers} = await run({frames:[event("finalizing"),event("completed")]});
  assert.deepEqual(result,{ok:true,signedUrl:"https://cdn.example.test/right.png?fixture=1",
    outputId:"output-1",actionId:"action-1",resolvedActionType:"create_image_uni_1",
    realmId:"realm-1",mediaOperation:"image",artifactField:"image",state:"completed"});
  const source = readers.find((r)=>r.kind==="events");
  assert.equal(source.cancelled,1); assert.equal(source.released,1);
  assert.ok(signals.every((s)=>s.aborted)); assert.equal(timers.size,0);
});
test("Luma only consumes a complete SSE event and accepts CRLF framing", async () => {
  const partial = await run({frames:[event("completed").trimEnd()]});
  assert.equal(partial.result.ok,false);
  const complete = await run({frames:[event("completed").replaceAll("\n","\r\n")]});
  assert.equal(complete.result.state,"completed");
});
test("Luma releases the subscribed stream when action submission fails", async () => {
  const {result,readers,signals} = await run({actionStatus:429});
  assert.equal(result.error.code,"lumalabs_action_failed");
  const source = readers.find((r)=>r.kind==="events");
  assert.equal(source?.cancelled,1); assert.equal(source?.released,1);
  assert.ok(signals.every((s)=>s.aborted));
});
test("Luma rejects ambiguous output IDs and HTML event sources before waiting", async () => {
  for (const action of [{output_artifacts:{image:[" "]}},{output_artifacts:{image:["output-1","two"]}}]) {
    const {result} = await run({action,frames:[event("completed")]});
    assert.equal(result.error.code,"lumalabs_missing_output_id");
  }
  const bad = await run({streamType:"text/html"});
  assert.equal(bad.result.ok,false);
  assert.equal(bad.calls.filter((c)=>c.url.endsWith("/actions")).length,0);
});
test("Luma metadata and cumulative SSE byte limits cannot be bypassed by complete chunks", async () => {
  const metadata = await run({signaturePadding:"x".repeat(128*1024)});
  assert.equal(metadata.result.error.code,"lumalabs_browser_metadata_limit");
  const events = await run({frames:[": "+"x".repeat(2*1024*1024)+"\n\n",event("completed")]});
  assert.equal(events.result.error.code,"lumalabs_browser_events_limit");
});
test("Luma does not turn cleanup failure or invalid UTF-8 into success", async () => {
  const cleanup = await run({frames:[event("completed")],cancelError:true});
  assert.equal(cleanup.result.ok,false);
  assert.equal(cleanup.result.error.code,"lumalabs_events_cleanup_failed");
  const utf8 = await run({frames:[Uint8Array.from([0xff]),event("completed")]});
  assert.equal(utf8.result.ok,false);
  assert.equal(utf8.result.error.code,"lumalabs_browser_fetch_failed");
  assert.ok(!JSON.stringify(cleanup.result).includes("private-cancel"));
});
test("Luma optional menus cannot hide metadata decode or cleanup failure", async () => {
  for (const options of [{cancelError:true,cancelKind:"menus"},{invalidMenus:true}]) {
    const {result,calls}=await run({...options,frames:[event("completed")],overrides:{autoDiscoverActionType:true}});
    assert.equal(result.ok,false);
    assert.equal(calls.filter((call)=>call.url.endsWith("/actions")).length,0);
    assert.ok(!JSON.stringify(result).includes("private-cancel"));
  }
});
