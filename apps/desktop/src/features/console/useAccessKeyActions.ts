import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import type { ConsoleApi } from "../../api/console";
import type {
  ConsoleAccessCatalog,
  ConsoleAccessKey,
} from "../../api/contracts";
import {
  ACCESS_DEFAULT_KEY_DRAFT,
  buildAccessKeyInput,
  type AccessKeyDraft,
  type AccessPanelState,
  type AccessRevealedSecret,
  type TranslateFn,
} from "./accessKeysTypes";
import { validKeyGroups } from "./accessKeyGroups";
import { editedAccessKeyInput } from "./accessKeyEditing";
import { validKeyQuota } from "./accessKeyQuota";
import { accessKeyDraftValid } from "./accessKeyPresentation";

type Options = {
  api: ConsoleApi;
  managementToken: string | null;
  active: boolean;
  t: TranslateFn;
  catalog: AccessPanelState<ConsoleAccessCatalog>;
  setCatalog: Dispatch<SetStateAction<AccessPanelState<ConsoleAccessCatalog>>>;
  loadCatalog(): Promise<void>;
  setRevealedSecret: Dispatch<SetStateAction<AccessRevealedSecret | null>>;
};

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

// All key mutations share one lock and identity epoch, including group edits.
export function useAccessKeyActions({
  api,
  managementToken,
  active,
  t,
  catalog,
  setCatalog,
  loadCatalog,
  setRevealedSecret,
}: Options) {
  const [keyDraft, setKeyDraft] = useState<AccessKeyDraft>({
    ...ACCESS_DEFAULT_KEY_DRAFT,
  });
  const [creatingKey, setCreatingKey] = useState(false);
  const [keyBusyId, setKeyBusyId] = useState<string | null>(null);
  const keyMutation = useRef(false);
  const keyEpoch = useRef(0);

  useEffect(() => {
    setRevealedSecret(null);
    setCreatingKey(false);
    setKeyBusyId(null);
    keyMutation.current = false;
    // Discard late plaintext after navigation, logout or an identity change.
    return () => {
      keyEpoch.current += 1;
    };
  }, [active, managementToken]);

  const updateKey = useCallback(
    async (key: ConsoleAccessKey, draft: AccessKeyDraft) => {
      if (
        !active ||
        !managementToken ||
        keyMutation.current ||
        !api.updateAccessKey ||
        !catalog.data ||
        !validKeyGroups(draft.accountGroupIds, catalog.data.accountGroups) ||
        !accessKeyDraftValid(draft) ||
        !validKeyQuota(draft)
      )
        return false;
      keyMutation.current = true;
      const epoch = keyEpoch.current;
      setKeyBusyId(key.id);
      try {
        await api.updateAccessKey(
          managementToken,
          key.id,
          editedAccessKeyInput(key, draft, catalog.data),
        );
        if (epoch !== keyEpoch.current) return false;
        await loadCatalog();
        return epoch === keyEpoch.current;
      } catch (cause) {
        if (epoch === keyEpoch.current)
          setCatalog((current) => ({ ...current, error: errorMessage(cause) }));
        return false;
      } finally {
        if (epoch === keyEpoch.current) {
          keyMutation.current = false;
          setKeyBusyId(null);
        }
      }
    },
    [active, api, catalog.data, loadCatalog, managementToken],
  );

  const copyKey = useCallback(
    async (id: string) => {
      if (
        !active ||
        !managementToken ||
        keyMutation.current ||
        !api.copyAccessKey
      )
        return false;
      keyMutation.current = true;
      const epoch = keyEpoch.current;
      setKeyBusyId(id);
      try {
        if (!navigator.clipboard?.writeText)
          throw new Error(
            t("当前环境无法写入剪贴板。", "Clipboard is unavailable."),
          );
        // Plaintext stays in this operation only, never in catalog, React state or browser storage.
        const response = await api.copyAccessKey(managementToken, id);
        if (epoch !== keyEpoch.current) return false;
        await navigator.clipboard.writeText(response.token);
        return epoch === keyEpoch.current;
      } catch (cause) {
        if (epoch === keyEpoch.current)
          setCatalog((current) => ({ ...current, error: errorMessage(cause) }));
        return false;
      } finally {
        if (epoch === keyEpoch.current) {
          keyMutation.current = false;
          setKeyBusyId(null);
        }
      }
    },
    [active, api, managementToken, t],
  );

  const createKey = useCallback(async () => {
    if (
      !active ||
      !managementToken ||
      keyMutation.current ||
      typeof api.createAccessKey !== "function"
    ) {
      return false;
    }
    keyMutation.current = true;
    const epoch = keyEpoch.current;
    setCreatingKey(true);
    try {
      const created = await api.createAccessKey(
        managementToken,
        buildAccessKeyInput(keyDraft),
      );
      if (epoch !== keyEpoch.current) return false;
      if (!created.token)
        throw new Error(
          t(
            "密钥已创建但未返回明文，请刷新列表后轮换。",
            "Key created without plaintext. Refresh the list and rotate it.",
          ),
        );
      setRevealedSecret({
        kind: "access-key",
        label: created.displayName,
        secret: created.token,
      });
      setKeyDraft({ ...ACCESS_DEFAULT_KEY_DRAFT });
      await loadCatalog();
      return true;
    } catch (cause) {
      if (epoch === keyEpoch.current) {
        setCatalog((current) => ({ ...current, error: errorMessage(cause) }));
      }
      return false;
    } finally {
      if (epoch === keyEpoch.current) {
        keyMutation.current = false;
        setCreatingKey(false);
      }
    }
  }, [active, api, keyDraft, loadCatalog, managementToken, t]);

  const rotateKey = useCallback(
    async (accessKeyId: string) => {
      if (
        !active ||
        !managementToken ||
        keyMutation.current ||
        typeof api.rotateAccessKey !== "function"
      ) {
        return false;
      }
      keyMutation.current = true;
      const epoch = keyEpoch.current;
      setKeyBusyId(accessKeyId);
      try {
        const rotated = await api.rotateAccessKey(managementToken, accessKeyId);
        if (epoch !== keyEpoch.current) return false;
        if (!rotated.token)
          throw new Error(
            t(
              "轮换未返回密钥明文，请刷新列表核对状态。",
              "Rotation returned no plaintext. Refresh the list to check its state.",
            ),
          );
        setRevealedSecret({
          kind: "access-key",
          label: rotated.displayName,
          secret: rotated.token,
        });
        await loadCatalog();
        return true;
      } catch (cause) {
        if (epoch === keyEpoch.current) {
          setCatalog((current) => ({ ...current, error: errorMessage(cause) }));
        }
        return false;
      } finally {
        if (epoch === keyEpoch.current) {
          keyMutation.current = false;
          setKeyBusyId(null);
        }
      }
    },
    [active, api, loadCatalog, managementToken, t],
  );

  const keyLifecycle = useCallback(
    async (accessKeyId: string, action: "enable" | "disable" | "delete") => {
      if (
        !active ||
        !managementToken ||
        keyMutation.current ||
        (action === "delete" ? !api.deleteAccessKey : !api.setAccessKeyEnabled)
      ) {
        return false;
      }
      keyMutation.current = true;
      const epoch = keyEpoch.current;
      setKeyBusyId(accessKeyId);
      try {
        if (action === "delete")
          await api.deleteAccessKey!(managementToken, accessKeyId);
        else
          await api.setAccessKeyEnabled!(
            managementToken,
            accessKeyId,
            action === "enable",
          );
        if (epoch !== keyEpoch.current) return false;
        await loadCatalog();
        return epoch === keyEpoch.current;
      } catch (cause) {
        if (epoch === keyEpoch.current) {
          setCatalog((current) => ({ ...current, error: errorMessage(cause) }));
        }
        return false;
      } finally {
        if (epoch === keyEpoch.current) {
          keyMutation.current = false;
          setKeyBusyId(null);
        }
      }
    },
    [active, api, loadCatalog, managementToken],
  );

  return {
    keyDraft,
    setKeyDraft,
    creatingKey,
    keyBusyId,
    createKey,
    rotateKey,
    keyLifecycle,
    updateKey,
    copyKey,
  };
}
