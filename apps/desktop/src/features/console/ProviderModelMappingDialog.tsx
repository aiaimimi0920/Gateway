import * as Dialog from "@radix-ui/react-dialog";
import { useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { ProviderModelMappingEntry } from "./providerModelMappingDocument";
export type { ProviderModelMappingEntry } from "./providerModelMappingDocument";

export type ProviderModelMappingDialogProps = {
  open: boolean;
  providerLabel: string;
  providerId: string;
  modelOptions: string[];
  upstreamModelOptions: string[];
  initialEntries: ProviderModelMappingEntry[];
  locked: boolean;
  savePending?: boolean;
  saveError?: string | null;
  onOpenChange(open: boolean): void;
  onSubmit(entries: ProviderModelMappingEntry[]): void;
};

/** The left selection owns its links; toggling right-hand targets only changes that source. */
export function ProviderModelMappingDialog({ open, providerId, providerLabel, modelOptions, upstreamModelOptions,
  initialEntries, locked, savePending = false, saveError = null, onOpenChange, onSubmit }: ProviderModelMappingDialogProps) {
  const { t } = useUiLocale();
  const returnFocus = useRef<HTMLElement | null>(null);
  const [entries, setEntries] = useState<ProviderModelMappingEntry[]>([]);
  const entriesRef = useRef<ProviderModelMappingEntry[]>([]);
  const [source, setSource] = useState("");
  const [query, setQuery] = useState("");
  const [targetQuery, setTargetQuery] = useState("");
  const [mappedOnly, setMappedOnly] = useState(false);
  const [customSource, setCustomSource] = useState("");
  const [customTarget, setCustomTarget] = useState("");
  const [extraSources, setExtraSources] = useState<string[]>([]);
  const [extraTargets, setExtraTargets] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!open) return;
    entriesRef.current = initialEntries.map((entry) => ({ ...entry }));
    setEntries(entriesRef.current);
    setSource(initialEntries[0]?.model ?? modelOptions[0] ?? "");
    setQuery(""); setTargetQuery(""); setMappedOnly(false); setError(null); setExtraSources([]); setExtraTargets([]);
    setCustomSource(""); setCustomTarget("");
    // Only reopening resets the editor; telemetry refresh must not overwrite local edits.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, providerId]);
  const sources = [...new Set([...modelOptions, ...entries.map((entry) => entry.model), ...extraSources])];
  const targets = [...new Set([...upstreamModelOptions, ...entries.map((entry) => entry.upstreamModel), ...extraTargets])];
  const selected = entries.filter((entry) => entry.model === source).map((entry) => entry.upstreamModel);
  const visibleSources = sources.filter((model) => model.toLowerCase().includes(query.toLowerCase())
    && (!mappedOnly || entries.some((entry) => entry.model === model)));
  const visibleTargets = targets.filter((target) => target.toLowerCase().includes(targetQuery.toLowerCase()));
  const validName = (name: string) => name.length > 0 && new TextEncoder().encode(name).length <= 256
    && !/[\x00-\x1f\x7f*?]/.test(name);
  const changeEntries = (next: ProviderModelMappingEntry[]) => {
    if (locked) return false;
    const mappings = new Map<string, number>();
    for (const entry of next) {
      if (!validName(entry.model) || !validName(entry.upstreamModel)) {
        setError(t("映射包含无效模型名。", "A mapping contains an invalid model name.")); return false;
      }
      mappings.set(entry.model, (mappings.get(entry.model) ?? 0) + 1);
    }
    if (mappings.size > 512 || [...mappings.values()].some((count) => count > 32)) {
      setError(t("每个模型最多关联 32 个目标，此池最多保存 512 个映射。", "Link at most 32 targets per model and 512 mappings per pool.")); return false;
    }
    setError(null);
    if (JSON.stringify(next) === JSON.stringify(entriesRef.current)) return true;
    // Commit only explicit edits, never mount/refresh; the controller owns debounced persistence.
    entriesRef.current = next;
    setEntries(next);
    onSubmit(next);
    return true;
  };
  const toggleTarget = (target: string, checked: boolean) => {
    if (!source || locked) return false;
    const next = entriesRef.current.filter((entry) => entry.model !== source || entry.upstreamModel !== target);
    return changeEntries(checked ? [...next, { model: source, upstreamModel: target }] : next);
  };
  const addName = (kind: "source" | "target") => {
    if (locked || (kind === "target" && !source)) return;
    const name = (kind === "source" ? customSource : customTarget).trim();
    if (!validName(name)) { setError(t("请输入不含通配符的模型名（最多 256 字节）。", "Enter an exact model name of at most 256 bytes, without wildcards.")); return; }
    if (kind === "source") {
      setError(null); setExtraSources((names) => [...new Set([...names, name])]);
      setSource(name); setQuery(""); setMappedOnly(false); setCustomSource("");
    } else if (toggleTarget(name, true)) {
      setExtraTargets((names) => [...new Set([...names, name])]); setTargetQuery(""); setCustomTarget("");
    }
  };
  const deleteSource = () => {
    if (!source || !changeEntries(entriesRef.current.filter((entry) => entry.model !== source))) return;
    setExtraSources((names) => names.filter((name) => name !== source));
    setSource(sources.find((name) => name !== source) ?? "");
  };
  const deleteTargets = () => {
    if (!source) return;
    const removed = customTarget.trim() ? [customTarget.trim()] : selected;
    const next = entriesRef.current.filter((entry) => entry.model !== source || !removed.includes(entry.upstreamModel));
    if (!changeEntries(next)) return;
    // Remove editor-only candidates, not provider capability declarations or another source's links.
    setExtraTargets((names) => names.filter((name) => !removed.includes(name) || next.some((entry) => entry.upstreamModel === name)));
    setCustomTarget("");
  };
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content nt-model-map-dialog" aria-describedby={undefined}
          onOpenAutoFocus={() => {
            returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            const trigger = returnFocus.current;
            if (trigger?.isConnected && !trigger.closest("[inert]")) trigger.focus();
          }}>
          <div className="nt-model-map-header">
            <Dialog.Title>{t(`模型映射 · ${providerLabel}`, `Model mapping · ${providerLabel}`)}</Dialog.Title>
            <Dialog.Close asChild><button className="nt-btn nt-btn--outline" type="button" aria-label={t("关闭", "Close")}>×</button></Dialog.Close>
          </div>
          <div className="nt-model-map-columns">
            <section className="nt-model-map-column" aria-label={t("用户模型", "Requested models")}>
              <div className="nt-model-map-toolbar"><h3>{t("用户模型", "Requested models")}</h3>
                <label><input type="checkbox" checked={mappedOnly} onChange={(event) => setMappedOnly(event.target.checked)} />{t("已映射", "Mapped")}</label>
              </div>
              <input className="nt-input" value={query} placeholder={t("搜索模型", "Search models")} aria-label={t("搜索用户模型", "Search requested models")} onChange={(event) => setQuery(event.target.value)} />
              <div className="nt-model-map-options">
                {visibleSources.map((model) => {
                  const linked = entries.filter((entry) => entry.model === model).map((entry) => entry.upstreamModel);
                  return <button className="nt-model-map-source" type="button" key={model} aria-pressed={source === model}
                    title={linked.length ? `${model} → ${linked.join(" / ")}` : model} aria-label={model} onClick={() => setSource(model)}>
                    <span><strong>{model}</strong><small>{linked.length ? `→ ${linked.join(" / ")}` : t("原名直传", "Pass through")}</small></span>
                  </button>;
                })}
                {!visibleSources.length ? <p className="nt-copy">{t("无匹配模型", "No matching models")}</p> : null}
              </div>
              <div className="nt-model-map-custom">
                <span>{t("自定义用户模型", "Custom requested model")}</span>
                <div className="nt-model-map-inline">
                  <input className="nt-input" aria-label={t("自定义用户模型", "Custom requested model")} value={customSource} maxLength={256} disabled={locked}
                    onChange={(event) => setCustomSource(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); addName("source"); } }} />
                  <button className="nt-btn nt-btn--outline" type="button" aria-label={t("添加用户模型", "Add requested model")} disabled={locked || !customSource.trim()} onClick={() => addName("source")}>{t("添加", "Add")}</button>
                  <button className="nt-btn nt-btn--outline" type="button" aria-label={t("删除用户模型", "Delete requested model")} disabled={locked || !source || (!selected.length && !extraSources.includes(source))} onClick={deleteSource}>{t("删除", "Delete")}</button>
                </div>
              </div>
            </section>
            <section className="nt-model-map-column" aria-label={t("凭据池模型", "Pool models")}>
              <div className="nt-model-map-toolbar"><h3>{t("凭据池模型", "Pool models")}</h3><small>{t(`已选 ${selected.length}`, `${selected.length} selected`)}</small></div>
              <input className="nt-input" value={targetQuery} placeholder={t("搜索模型", "Search models")} aria-label={t("搜索凭据池模型", "Search pool models")} onChange={(event) => setTargetQuery(event.target.value)} />
              <div className="nt-model-map-options">
                {visibleTargets.map((target) => <label className="nt-model-map-target" key={target} title={target}>
                  <input type="checkbox" checked={selected.includes(target)} disabled={locked || !source}
                    onChange={(event) => toggleTarget(target, event.target.checked)} /><span>{target}</span>
                </label>)}
                {!visibleTargets.length ? <p className="nt-copy">{targets.length ? t("无匹配模型", "No matching models") : t("暂无配置模型", "No configured models")}</p> : null}
              </div>
              <div className="nt-model-map-custom">
                <span>{t("自定义映射模型", "Custom mapped model")}</span>
                <div className="nt-model-map-inline">
                  <input className="nt-input" aria-label={t("自定义映射模型", "Custom mapped model")} value={customTarget} maxLength={256} disabled={locked || !source}
                    onChange={(event) => setCustomTarget(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); addName("target"); } }} />
                  <button className="nt-btn nt-btn--outline" type="button" aria-label={t("添加映射模型", "Add mapped model")} disabled={locked || !source || !customTarget.trim()} onClick={() => addName("target")}>{t("添加", "Add")}</button>
                  <button className="nt-btn nt-btn--outline" type="button" aria-label={t("删除映射模型", "Delete mapped models")} disabled={locked || !source || (!selected.length && !extraTargets.includes(customTarget.trim()))} onClick={deleteTargets}>{t("删除", "Delete")}</button>
                </div>
              </div>
            </section>
          </div>
          {error || saveError ? <p className="nt-banner nt-banner--danger" role="alert">{error || saveError}</p> : null}
          {savePending ? <span className="nt-copy" role="status">{t("保存中…", "Saving…")}</span> : null}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
