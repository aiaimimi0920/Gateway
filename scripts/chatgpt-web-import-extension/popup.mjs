import { captureSelectedTab } from "./capture.mjs";

const consent = document.getElementById("consent");
const button = document.getElementById("export");
const status = document.getElementById("status");
consent.addEventListener("change", () => { button.disabled = !consent.checked; });
document.getElementById("open").addEventListener("click", () => chrome.tabs.create({ url: "https://chatgpt.com/" }));
button.addEventListener("click", async () => {
  if (!consent.checked) return;
  button.disabled = true; consent.disabled = true;
  status.textContent = "正在读取当前网页会话…";
  let url;
  try {
    const material = await captureSelectedTab(chrome);
    url = URL.createObjectURL(new Blob([JSON.stringify(material)], { type: "application/json" }));
    const link = document.createElement("a");
    link.href = url; link.download = "gateway-chatgpt-web-session.json";
    document.body.append(link); link.click(); link.remove();
    status.textContent = "已请求下载。请在 Gateway 的网页会话 JSON 中选择该文件。";
  } catch {
    // Never echo exceptions containing session responses, cookies, or tokens.
    status.textContent = "导出失败。请确认当前页是已登录的 ChatGPT，刷新后重试。";
  } finally {
    if (url) setTimeout(() => URL.revokeObjectURL(url), 1000);
    consent.checked = false; consent.disabled = false;
  }
});
