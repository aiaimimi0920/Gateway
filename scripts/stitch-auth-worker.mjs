/** Stitch-only normal login. No full browser-state export or runtime browser fallback. */
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { openManualLoginBrowser } from "./aistudio-manual-login-browser.mjs";

const ORIGIN = "https://stitch.withgoogle.com";
export function bundleFromCookies(cookies) {
  if (!Array.isArray(cookies) || cookies.length > 256) throw new Error("stitch_cookie_limit");
  const selected = cookies.filter(c => [".withgoogle.com","stitch.withgoogle.com",".stitch.withgoogle.com"].includes(c.domain)
    && c.partitionKey == null && (!/^(?:__Secure-|__Host-)/.test(c.name) || c.secure)
    && (!c.name.startsWith("__Host-") || !c.domain.startsWith(".") && c.path === "/")
    && (c.expires === -1 || c.expires > Date.now()/1000)).map(c => ({name:c.name,value:c.value,domain:c.domain,
      path:c.path,expires:c.expires,secure:c.secure,httpOnly:c.httpOnly,sameSite:c.sameSite}));
  if (!selected.some(c => c.value && c.path === "/")) throw new Error("stitch_login_session_missing");
  const bundle = {schemaVersion:1,provider:"stitch-web",authUser:"0",cookies:selected};
  if (selected.length > 64 || Buffer.byteLength(JSON.stringify(bundle)) > 32768) throw new Error("stitch_cookie_limit");
  return bundle;
}

export async function captureSession({tempRoot}, dependencies = {}) {
  if (typeof tempRoot !== "string" || !path.isAbsolute(tempRoot)) throw new Error("stitch_temp_root_required");
  const {chromium} = dependencies.chromium ? dependencies : await import("playwright-core");
  const executablePath = dependencies.executablePath ?? process.env.STITCH_CAPTURE_BROWSER_EXECUTABLE_PATH ?? [
    "C:/Program Files/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
  ].find(file=>fs.existsSync(file));
  if (!executablePath) throw new Error("stitch_browser_not_found");
  const owner = await (dependencies.open ?? openManualLoginBrowser)({chromium,executablePath,targetUrl:ORIGIN+"/",tempRoot,
    disableBackgroundMode:true,progress:dependencies.progress ?? (()=>{})});
  try {
    const wait = dependencies.wait ?? (ms=>new Promise(resolve=>setTimeout(resolve,ms)));
    const deadline = Date.now()+20000;
    while (Date.now()<deadline) {
      const page=owner.context.pages().find(p=>{try{return new URL(p.url()).origin===ORIGIN;}catch{return false;}});
      if (page) {
        await page.waitForLoadState("domcontentloaded",{timeout:20000});
        return bundleFromCookies(await owner.context.cookies([ORIGIN+"/"]));
      }
      // CDP can attach before Chrome creates/navigates the requested tab. Do not misclassify that race as logout.
      await wait(100);
    }
    throw new Error("stitch_login_page_missing");
  } finally { await owner.close(); }
}

async function main() {
  let input="";
  for await (const chunk of process.stdin) {input+=chunk;if(Buffer.byteLength(input)>4096)throw new Error("stitch_import_input_limit");}
  process.stdout.write(JSON.stringify(await captureSession(JSON.parse(input))));
}
if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href===import.meta.url) {
  main().catch(()=>{process.stdout.write(JSON.stringify({ok:false,code:"stitch_import_failed"}));process.exitCode=1;});
}
