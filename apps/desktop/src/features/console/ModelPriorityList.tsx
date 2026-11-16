import { useRef, useState } from "react";
import { GripVertical, ArrowUp, ArrowDown } from "lucide-react";
import { Button } from "@neuro/ui";
import type { ModelPoolProviderLink } from "./modelPoolViewModel";

type Props = {
  chain: ModelPoolProviderLink[];
  locked: boolean;
  t: (zh: string, en: string) => string;
  onMove: (id: string, direction: "up" | "down" | number) => void;
  onAccounts: (id: string) => void;
};

/** Commit once on drop. Never replay intermediate swaps against an autosaving draft. */
export function ModelPriorityList({ chain, locked, t, onMove, onAccounts }: Props) {
  const host = useRef<HTMLTableElement>(null);
  const drag = useRef<{ id: string; signature: string; y: number; moved: boolean } | null>(null);
  const [target, setTarget] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const signature = JSON.stringify(chain.map(link => link.providerId));
  const cancel = () => { drag.current = null; setTarget(null); };
  return <>
    <p className="nt-visually-hidden" role="status">{announcement}</p>
    <table ref={host} className="nt-directory-table nt-priority-table" aria-label={t("供应商优先级", "Provider priority")}>
      <thead><tr><th>{t("顺序", "Order")}</th><th>{t("供应商", "Provider")}</th><th>{t("关联账号", "Accounts")}</th><th>{t("调整", "Adjust")}</th></tr></thead>
      <tbody>{chain.map((link, index) => <tr key={link.providerId} data-priority-provider={link.providerId} data-drop-target={target === link.providerId}>
        <td><button type="button" className="nt-priority-grip" disabled={locked || chain.length < 2}
          aria-label={t(`拖动 ${link.providerLabel} 调整优先级`, `Drag ${link.providerLabel} to reorder`)}
          onPointerDown={event => {
            if (locked || event.button !== 0) return;
            drag.current = { id: link.providerId, signature, y: event.clientY, moved: false };
            event.currentTarget.setPointerCapture(event.pointerId);
          }}
          onPointerMove={event => {
            if (!drag.current) return;
            if (Math.abs(event.clientY - drag.current.y) < 4 && !drag.current.moved) return;
            drag.current.moved = true;
            const row = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-priority-provider]");
            setTarget(row && host.current?.contains(row) ? row.dataset.priorityProvider ?? null : null);
          }}
          onPointerUp={event => {
            const current = drag.current;
            const row = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-priority-provider]");
            const to = row && host.current?.contains(row) ? chain.findIndex(item => item.providerId === row.dataset.priorityProvider) : -1;
            cancel();
            if (!locked && current?.moved && current.signature === signature && to >= 0 && chain[to].providerId !== current.id) {
              onMove(current.id, to);
              setAnnouncement(t(`已调整至第 ${to + 1} 位`, `Moved to position ${to + 1}`));
            }
            if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
          }}
          onPointerCancel={cancel} onLostPointerCapture={cancel}
          onKeyDown={event => {
            if (event.key === "Escape") cancel();
            if (!locked && ["ArrowUp", "ArrowDown"].includes(event.key)) {
              event.preventDefault();
              onMove(link.providerId, event.key === "ArrowUp" ? "up" : "down");
            }
          }}><GripVertical size={15} /></button><span>{index + 1}</span></td>
        <td>{link.providerLabel}{link.detached && <small className="n-muted"> · {t("无关联账号", "No linked accounts")}</small>}</td>
        <td><Button compact variant="quiet" onClick={() => onAccounts(link.providerId)}>{link.accountCount}</Button></td>
        <td><div className="nt-directory-actions"><button type="button" disabled={locked || index === 0} aria-label={t(`上移 ${link.providerLabel}`, `Move ${link.providerLabel} up`)} onClick={() => onMove(link.providerId, "up")}><ArrowUp size={13} /></button>
          <button type="button" disabled={locked || index === chain.length - 1} aria-label={t(`下移 ${link.providerLabel}`, `Move ${link.providerLabel} down`)} onClick={() => onMove(link.providerId, "down")}><ArrowDown size={13} /></button></div></td>
      </tr>)}</tbody>
    </table>
  </>;
}
