import { Plus } from "lucide-react";
import { AccessKeyHeaderContent, AccessKeyToolbar, compareAccessKeys, type AccessKeySort } from "./AccessKeyHeader";
import { useMemo, useRef, useState } from "react";
import { useGatewayHost } from "../../platform/HostProvider";
import {
  AccessKeyDialog,
  AccessKeyConfirmDialog,
  type PendingKeyAction,
} from "./AccessKeyDialogs";
import { AccessKeyLedger } from "./AccessKeyLedger";
import { AccessKeyCashDialog } from "./AccessKeyCashDialog";
import { AccessKeyEditDialog } from "./AccessKeyEditDialog";
import type { ConsoleAccessKey } from "../../api/contracts";
import { CopyButton } from "./AccessWorkspacePrimitives";
import { newAccessKeyDraft } from "./accessKeyPresentation";
import type { AccessKeysWorkspaceProps } from "./accessKeysTypes";
import "../../styles/access-keys.css";

export type AccessKeysSectionProps = Pick<
  AccessKeysWorkspaceProps,
  | "t"
  | "catalog"
  | "editorLocked"
  | "refreshing"
  | "onRefresh"
  | "keyDraft"
  | "onKeyDraftChange"
  | "onCreateKey"
  | "creatingKey"
  | "keyBusyId"
  | "onRotateKey"
  | "onKeyLifecycle"
  | "onUpdateKey"
  | "onCopyKey"
  | "revealedSecret"
  | "onDismissSecret"
>;

const PAGE_SIZE = 20;

// Only UI state lives here; issuance, revocation and secret lifetime remain in useAccessData.
export function AccessKeysSection(props: AccessKeysSectionProps) {
  const {
    t,
    catalog,
    editorLocked,
    creatingKey,
    keyBusyId,
    revealedSecret,
    onDismissSecret,
  } = props;
  const { apiOrigin } = useGatewayHost();
  const apiBaseUrl = apiOrigin.replace(/\/$/, "") + "/v1";
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<AccessKeySort>("newest");
  const [page, setPage] = useState(0);
  const [creating, setCreating] = useState(false);
  const [pending, setPending] = useState<PendingKeyAction | null>(null);
  const [editingKey, setEditingKey] = useState<ConsoleAccessKey | null>(null);
  const [billingKey, setBillingKey] = useState<ConsoleAccessKey | null>(null);
  const createButton = useRef<HTMLButtonElement>(null);
  const busy = creatingKey || keyBusyId !== null;
  const rows = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    return (catalog.data?.accessKeys ?? [])
      .filter(
        (key) =>
          [key.displayName, key.id, key.ownerId, key.publicKeyPrefix].some(
            (value) => value.toLocaleLowerCase().includes(needle),
          ),
      )
      .sort((left, right) => compareAccessKeys(left, right, sort));
  }, [catalog.data, query, sort]);
  const pages = Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
  const currentPage = Math.min(page, pages - 1);
  const close = () => {
    setCreating(false);
    onDismissSecret();
  };
  const confirm = async () => {
    if (!pending || busy || editorLocked) return;
    const success = await (pending.kind === "rotate"
      ? props.onRotateKey(pending.key.id)
      : props.onKeyLifecycle(pending.key.id, pending.kind));
    if (success) setPending(null);
  };

  return (
    <section className="nt-key-manager" aria-label="API Keys">
      <AccessKeyHeaderContent slot="leading">
        <AccessKeyToolbar query={query} sort={sort} t={t}
          onQuery={(value) => { setQuery(value); setPage(0); }}
          onSort={(value) => { setSort(value); setPage(0); }} />
      </AccessKeyHeaderContent>
      <AccessKeyHeaderContent slot="actions">
        <div className="nt-key-header-actions">
          <span className="nt-key-header-url"><code title={apiBaseUrl}>{apiBaseUrl}</code></span>
          <CopyButton t={t} value={apiBaseUrl} label={t("复制", "Copy")} variant="outline" />
          <button
            ref={createButton}
            className="nt-btn nt-btn--primary"
            type="button"
            disabled={
              editorLocked || busy || !catalog.data || Boolean(revealedSecret)
            }
            onClick={() => {
              props.onKeyDraftChange(newAccessKeyDraft(catalog.data));
              setCreating(true);
            }}
          >
            <Plus size={16} aria-hidden="true" />
            {t("新建 Key", "New Key")}
          </button>
        </div>
      </AccessKeyHeaderContent>
      {catalog.error ? (
        <div className="nt-alert nt-alert--danger" role="alert">
          {catalog.error}
        </div>
      ) : null}
      {catalog.loading && !catalog.data ? (
        <div className="nt-key-empty" role="status">
          {t("正在读取密钥…", "Loading keys…")}
        </div>
      ) : rows.length === 0 ? (
        <div className="nt-key-empty" role="status">
          {catalog.error && !catalog.data
            ? t("密钥列表不可用", "Key list unavailable")
            : query
              ? t("没有匹配的密钥", "No matching keys")
              : t("暂无 API Key", "No API Keys yet")}
        </div>
      ) : (
        <AccessKeyLedger
          keys={rows.slice(
            currentPage * PAGE_SIZE,
            (currentPage + 1) * PAGE_SIZE,
          )}
          catalog={catalog.data!}
          locked={editorLocked || busy || Boolean(revealedSecret)}
          onAction={setPending}
          onEdit={setEditingKey}
          onBills={setBillingKey}
          onCopy={props.onCopyKey}
          t={t}
        />
      )}
      {rows.length > PAGE_SIZE ? (
        <div className="nt-key-pagination">
          <span>
            {currentPage + 1} / {pages} · {rows.length}
          </span>
          <button
            className="nt-btn nt-btn--outline"
            type="button"
            disabled={currentPage === 0}
            onClick={() => setPage(currentPage - 1)}
          >
            {t("上一页", "Previous")}
          </button>
          <button
            className="nt-btn nt-btn--outline"
            type="button"
            disabled={currentPage + 1 >= pages}
            onClick={() => setPage(currentPage + 1)}
          >
            {t("下一页", "Next")}
          </button>
        </div>
      ) : null}
      {billingKey ? (
        <AccessKeyCashDialog
          accessKey={billingKey}
          onClose={() => setBillingKey(null)}
          t={t}
        />
      ) : null}
      <AccessKeyDialog
        {...props}
        open={creating}
        onClose={close}
        onRestoreFocus={() => createButton.current?.focus()}
        apiBaseUrl={apiBaseUrl}
        revealedSecret={pending ? null : revealedSecret}
      />
      <AccessKeyConfirmDialog
        pending={pending}
        busy={busy || editorLocked}
        error={catalog.error}
        onClose={() => setPending(null)}
        onConfirm={() => {
          void confirm();
        }}
        t={t}
      />
      {editingKey && catalog.data ? (
        <AccessKeyEditDialog
          key={editingKey.id}
          accessKey={editingKey}
          catalog={catalog.data}
          busy={busy || editorLocked}
          error={catalog.error}
          onClose={() => setEditingKey(null)}
          onSave={props.onUpdateKey}
          t={t}
        />
      ) : null}
    </section>
  );
}
