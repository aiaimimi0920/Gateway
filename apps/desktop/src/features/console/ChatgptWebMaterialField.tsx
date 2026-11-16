import { useEffect, useRef, useState } from "react";
import { Button, Notice } from "@neuro/ui";
import { parseChatgptWebMaterial, WEB_MATERIAL_MAX_BYTES, type ChatgptWebMaterial } from "./chatgptWebMaterial";

/** File contents stay in transient dialog state, never localStorage or the JSON editor. */
export function ChatgptWebMaterialField({ disabled, loaded, onMaterial, onClear }: {
  disabled: boolean;
  loaded: boolean;
  onMaterial(material: ChatgptWebMaterial): void;
  onClear(): void;
}) {
  const generation = useRef(0);
  const fileInput = useRef<HTMLInputElement | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    generation.current += 1;
    setBusy(false);
    setError(null);
    return () => { generation.current += 1; };
  }, [disabled]);
  return <div className="n-stack">
    <Button disabled={disabled || busy} loading={busy} onClick={() => fileInput.current?.click()}>网页会话 JSON</Button>
    <input ref={fileInput} hidden type="file" aria-label="网页会话 JSON" accept=".json,application/json" disabled={disabled || busy}
      onChange={async (event) => {
        const file = event.currentTarget.files?.[0];
        event.currentTarget.value = "";
        if (!file || disabled) return;
        onClear();
        const request = ++generation.current;
        setError(null);
        if (file.size > WEB_MATERIAL_MAX_BYTES) { setError("网页会话文件不能超过 128 KiB。"); return; }
        setBusy(true);
        try {
          const source = await file.text();
          if (request !== generation.current) return;
          onMaterial(parseChatgptWebMaterial(source));
        } catch (cause) {
          if (request === generation.current) setError(cause instanceof Error && cause.message.startsWith("网页会话")
            ? cause.message : "网页会话导入失败，请检查材料格式；此入口不接收 OAuth 刷新凭据。");
        } finally { if (request === generation.current) setBusy(false); }
      }} />
    {loaded && <Notice>网页会话材料已读取，待保存</Notice>}
    {error && <Notice tone="danger">{error}</Notice>}
  </div>;
}
