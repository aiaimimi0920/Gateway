import { Button, Field, Notice, Textarea } from "@neuro/ui";
import { useEffect, useRef, useState } from "react";
import { parseProviderSessionMaterial, SESSION_MATERIAL_MAX_BYTES, type ProviderSessionMaterial } from "./providerSessionMaterial";
import { SESSION_PRODUCTS, type SessionProduct } from "./providerSessionProducts";

/** Plaintext exists only in this transient, grant-gated dialog; late file reads cannot cross editing sessions. */
export function ProviderSessionMaterialField({ product, disabled, loaded, resetKey, onMaterial, onClear }: {
  product: SessionProduct;
  disabled: boolean;
  loaded: boolean;
  resetKey: unknown;
  onMaterial(material: ProviderSessionMaterial): void;
  onClear(): void;
}) {
  const generation = useRef(0);
  const fileInput = useRef<HTMLInputElement | null>(null);
  const [source, setSource] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    generation.current += 1;
    setSource(""); setError(null); setBusy(false);
    return () => { generation.current += 1; };
  }, [disabled, product, resetKey]);
  const read = (value: string) => {
    try { onMaterial(parseProviderSessionMaterial(value, product)); setError(null); }
    catch (cause) { setError(cause instanceof Error && cause.message.startsWith("会话材料") ? cause.message : "会话材料读取失败；请检查格式。"); }
    finally { setSource(""); }
  };
  const example = {
    product, ...(product === "suno-web" ? { cookieHeader: "__session=...; other=..." } : { token: "..." }),
    ...(product === "perplexity-web" ? { userAgent: "原会话 User-Agent" } : {}),
    ...(product === "lumalabs-web" ? { realmId: "..." } : {}), ...(product === "freebuff-web" ? { agentId: "..." } : {})
  };
  return <div className="nt-stack">
    <p>{SESSION_PRODUCTS[product].hint}</p>
    <Field label="会话材料 JSON">
      <Textarea compact rows={4} value={source} disabled={disabled || busy} autoComplete="off" spellCheck={false}
        placeholder={JSON.stringify(example)} onChange={(event) => {
          generation.current += 1; onClear(); setError(null);
          const text = event.currentTarget.value;
          if (new TextEncoder().encode(text).length > SESSION_MATERIAL_MAX_BYTES) { setSource(""); setError("会话材料不能超过 128 KiB。"); }
          else setSource(text);
        }} /></Field>
    <div className="nt-actions">
      <Button compact disabled={disabled || busy || !source.trim()}
        onClick={() => { if (!disabled && !busy) { onClear(); read(source); } }}>读取会话材料</Button>
      <Button compact disabled={disabled || busy} loading={busy} onClick={() => fileInput.current?.click()}>导入会话文件</Button>
    </div>
    <input ref={fileInput} hidden aria-label="导入会话文件" type="file" accept=".json,application/json" disabled={disabled || busy} onChange={async (event) => {
      const file = event.currentTarget.files?.[0]; event.currentTarget.value = "";
      if (!file || disabled || busy) return;
      onClear(); setSource(""); setError(null);
      const request = ++generation.current;
      if (file.size > SESSION_MATERIAL_MAX_BYTES) { setError("会话材料不能超过 128 KiB。"); return; }
      setBusy(true);
      try { const text = await file.text(); if (request === generation.current) read(text); }
      catch { if (request === generation.current) setError("会话材料文件读取失败。"); }
      finally { if (request === generation.current) setBusy(false); }
    }} />
    {loaded && <Notice tone="success">会话材料已读取，待保存</Notice>}
    {error && <Notice tone="danger">{error}</Notice>}
  </div>;
}
