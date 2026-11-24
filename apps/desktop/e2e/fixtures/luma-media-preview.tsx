// 真实媒体表单的合成预览；回调只记录请求，不调用管理 API 或供应商。
import { useState } from "react";
import { createRoot } from "react-dom/client";
import { Button } from "@neuro/ui";
import type { ConsoleRouteDocument } from "../../src/api/contracts";
import { UiThemeProvider } from "../../src/theme/UiThemeProvider";
import { LumaMediaProbeDialog } from "../../src/features/console/LumaMediaProbeDialog";
import { LumaTaskPreview } from "./luma-task-preview";
import { LumaMcpCreationPreview } from "./luma-mcp-creation-preview";
import "../../src/styles.css";
import "@neuro/ui/styles.css";

const document: ConsoleRouteDocument = {
  providers: [{
    id: "preview-luma",
    preset: "lumalabs-api",
    base_url: "https://agents.lumalabs.ai/v1",
    supported_models: ["uni-1", "ray-3.2"],
    credentials: [{
      id: "synthetic",
      name: "合成账号",
      supported_models: ["uni-1", "ray-3.2"],
    }],
  }],
  model_routes: [],
  aliases: {},
};
const syntheticWindow = window as unknown as { syntheticRequests: unknown[] };
syntheticWindow.syntheticRequests = [];

function Preview() {
  const [open, setOpen] = useState(true);
  return (
    <UiThemeProvider initialMode="dark">
      <main style={{ padding: 24 }}>
        <p>合成预览：只记录表单请求，不连接服务商。</p>
        <Button onClick={() => setOpen(true)}>打开媒体测试</Button>
        <LumaTaskPreview />
        <LumaMcpCreationPreview />
        {open && (
          <LumaMediaProbeDialog
            document={document}
            providerId="preview-luma"
            credentialId="synthetic"
            t={zh => zh}
            onClose={() => setOpen(false)}
            locked={false}
            draftReady
            error={null}
            onProbe={async request => {
              syntheticWindow.syntheticRequests.push(request);
              throw new Error("合成预览：已记录请求，未连接服务商。");
            }}
          />
        )}
      </main>
    </UiThemeProvider>
  );
}

createRoot(window.document.getElementById("root")!).render(<Preview />);
