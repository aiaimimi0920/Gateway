import test from "node:test";
import assert from "node:assert/strict";
import { captureSession, sessionFromCookies, validateMaterial } from "../flowmusic-auth-worker.mjs";

test("imports only FlowMusic Supabase session, including split URL/base64 cookies", () => {
  const value = `base64-${Buffer.from(JSON.stringify({access_token:"fixture.access.token",refresh_token:"refresh-fixture"})).toString("base64url")}`;
  const mid = Math.floor(value.length / 2);
  const cookies = [
    {domain:"www.flowmusic.app",name:"sb-sb-auth-token.1",value:encodeURIComponent(value.slice(mid))},
    {domain:".flowmusic.app",name:"sb-sb-auth-token.0",value:encodeURIComponent(value.slice(0,mid))},
    {domain:"accounts.google.com",name:"SID",value:"never-import"},
  ];
  assert.deepEqual(sessionFromCookies(cookies), {schemaVersion:1,provider:"flowmusic-web",accessToken:"fixture.access.token",refreshToken:"refresh-fixture"});
  assert.throws(() => sessionFromCookies([{domain:"foreign.example",name:"sb-sb-auth-token",value}]));
});

test("rejects full browser states, unknown fields, header injection and unbounded secrets", () => {
  const valid = {schemaVersion:1,provider:"flowmusic-web",accessToken:"fixture.access.token"};
  assert.equal(validateMaterial(valid).provider,"flowmusic-web");
  for (const bad of [{...valid,origins:[]},{...valid,provider:"gemini-web"},{...valid,accessToken:"a\r\nb"},
    {...valid,accessToken:"a".repeat(17000)}]) assert.throws(() => validateMaterial(bad));
});

test("closes the dedicated import browser before returning or rejecting, without generation", async () => {
  for (const success of [true, false]) {
    let closed = 0, reads = 0;
    const material = await captureSession({tempRoot:"C:/fixture"}, {chromium:{},executablePath:"C:/fixture.exe",
      open: async options => {
        assert.equal(options.targetUrl,"https://www.flowmusic.app/");
        assert.equal(options.disableBackgroundMode,true);
        return {context:{cookies:async origins => {
          reads++; assert.deepEqual(origins,["https://www.flowmusic.app/","https://flowmusic.app/"]);
          return success ? [{domain:"www.flowmusic.app",name:"sb-sb-auth-token",value:JSON.stringify({access_token:"fixture.access.token"})}] : [];
        }},close:async () => {closed++;}};
      }}).catch(error => error);
    assert.equal(closed,1); assert.equal(reads,1);
    if (success) assert.equal(material.provider,"flowmusic-web"); else assert.ok(material instanceof Error);
  }
});
