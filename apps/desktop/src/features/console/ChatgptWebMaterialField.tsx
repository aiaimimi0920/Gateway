import { useEffect, useRef, useState } from "react";
import { parseChatgptWebMaterial, WEB_MATERIAL_MAX_BYTES, type ChatgptWebMaterial } from "./chatgptWebMaterial";

/** File contents stay in transient dialog state, never localStorage or the JSON editor. */
export function ChatgptWebMaterialField({ disabled, loaded, onMaterial, onClear }: {
  disabled: boolean;
  loaded: boolean;
  onMaterial(material: ChatgptWebMaterial): void;
  onClear(): void;
}) {
  const generation = useRef(0);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    generation.current += 1;
    setBusy(false);
    setError(null);
    return () => { generation.current += 1; };
  }, [disabled]);
  return <div className="nt-stack">
    <label className="nt-credential-row">
      <span>网页会话 JSON</span>
      <input type="file" accept=".json,application/json" disabled={disabled || busy}
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
    </label>
    {loaded && <span role="status">网页会话材料已读取，待保存</span>}
    {error && <span role="alert">{error}</span>}
  </div>;
}
