import {
  createContext,
  type ReactNode,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { createGatewayApiClient } from "../api/client";
import { createConsoleApi, type ConsoleApi } from "../api/console";
import type { BootstrapStatus, ManagementSession, SecretGrant } from "../api/contracts";
import { isAuthenticationError } from "../api/errors";
import { useGatewayHost } from "../platform/HostProvider";
import { rotateManagementToken } from "./managementTokenRotation";
import { useAutomaticSecretAccess } from "./useAutomaticSecretAccess";
import {
  clearManagementSession,
  getManagementSessionStorage,
  readManagementSessionToken,
  writeManagementSessionToken,
} from "./storage";

export type ManagementSessionPhase =
  | "checking"
  | "bootstrap-required"
  | "unauthenticated"
  | "authenticated"
  | "error";

export type ManagementSessionContextValue = {
  phase: ManagementSessionPhase;
  bootstrapStatus: BootstrapStatus | null;
  session: ManagementSession | null;
  managementToken: string | null;
  secretGrant: SecretGrant | null;
  busy: boolean;
  error: string | null;
  bootstrap(token: string): Promise<void>;
  login(token: string): Promise<void>;
  logout(): Promise<void>;
  rotate(newToken: string): Promise<void>;
  confirmSecretAccess(confirmationToken: string): Promise<void>;
  clearSecretGrant(): void;
  retryInitialization(): Promise<void>;
};

export const ManagementSessionContext = createContext<ManagementSessionContextValue | undefined>(
  undefined,
);

export type ManagementSessionProviderProps = {
  children: ReactNode;
  api?: ConsoleApi;
  storage?: Storage;
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function normalizeToken(value: string): string {
  const token = value.trim();
  if (!token) {
    throw new Error("Management token cannot be empty.");
  }
  return token;
}

export function ManagementSessionProvider({
  children,
  api: suppliedApi,
  storage: suppliedStorage,
}: ManagementSessionProviderProps) {
  const host = useGatewayHost();
  const storage = suppliedStorage ?? getManagementSessionStorage();
  const sessionNamespace = host.apiOrigin;
  const hostEpoch = useMemo(() => ({ namespace: sessionNamespace }), [sessionNamespace]);
  const hostEpochRef = useRef(hostEpoch);
  const [phase, setPhase] = useState<ManagementSessionPhase>("checking");
  const [bootstrapStatus, setBootstrapStatus] = useState<BootstrapStatus | null>(null);
  const [session, setSession] = useState<ManagementSession | null>(null);
  const [managementToken, setManagementToken] = useState<string | null>(null);
  const [secretGrant, setSecretGrant] = useState<SecretGrant | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const credentialIntent = useRef(0);
  const currentToken = useRef<string | null>(null);
  useLayoutEffect(() => { currentToken.current = managementToken; }, [managementToken]);

  const isCurrentHost = useCallback(() => hostEpochRef.current === hostEpoch, [hostEpoch]);

  const clearLocalSession = useCallback(() => {
    clearManagementSession(storage, hostEpoch.namespace);
    if (!isCurrentHost()) {
      return;
    }
    currentToken.current = null;
    setManagementToken(null);
    setSession(null);
    setSecretGrant(null);
    setPhase("unauthenticated");
  }, [hostEpoch.namespace, isCurrentHost, storage]);

  const clearSecretAccess = useCallback(() => {
    if (!isCurrentHost()) {
      return;
    }
    setSecretGrant(null);
    setSession((current) =>
      current ? { ...current, secretAccessGranted: false } : current,
    );
  }, [isCurrentHost]);

  const client = useMemo(
    () =>
      createGatewayApiClient({
        host,
        onAuthenticationFailure: (_status, token) => {
          // An old in-flight request must not invalidate a newly rotated credential.
          if (token && token !== currentToken.current) return;
          clearLocalSession();
        },
      }),
    [clearLocalSession, host],
  );
  const defaultApi = useMemo(() => createConsoleApi(client), [client]);
  const api = suppliedApi ?? defaultApi;

  const initialize = useCallback(
    async (isActive: () => boolean = () => true) => {
      const isActiveHost = () => isActive() && isCurrentHost();
      setBusy(true);
      setPhase("checking");
      setError(null);
      try {
        const status = await api.getBootstrapStatus();
        if (!isActiveHost()) {
          return;
        }
        setBootstrapStatus(status);
        if (status.needsBootstrap) {
          clearManagementSession(storage, hostEpoch.namespace);
          setManagementToken(null);
          setSession(null);
          setSecretGrant(null);
          setPhase("bootstrap-required");
          return;
        }

        const storedToken = readManagementSessionToken(storage, hostEpoch.namespace);
        if (!storedToken) {
          setManagementToken(null);
          setSession(null);
          setSecretGrant(null);
          setPhase("unauthenticated");
          return;
        }
        const verified = await api.verifySession(storedToken);
        if (!isActiveHost()) {
          return;
        }
        setManagementToken(storedToken);
        setSession(verified);
        setPhase("authenticated");
      } catch (cause) {
        if (!isActiveHost()) {
          return;
        }
        if (isAuthenticationError(cause)) {
          clearLocalSession();
          return;
        }
        setError(errorMessage(cause));
        setPhase("error");
      } finally {
        if (isActiveHost()) {
          setBusy(false);
        }
      }
    },
    [api, clearLocalSession, hostEpoch.namespace, isCurrentHost, storage],
  );

  useLayoutEffect(() => {
    hostEpochRef.current = hostEpoch;
    setBootstrapStatus(null);
    setManagementToken(null);
    setSession(null);
    setSecretGrant(null);
    setBusy(false);
    setError(null);
    setPhase("checking");
  }, [hostEpoch]);

  useEffect(() => {
    let active = true;
    void initialize(() => active);
    return () => {
      active = false;
    };
  }, [initialize]);

  const authenticateWith = useCallback(
    async (tokenValue: string, bootstrapFirst: boolean) => {
      const token = normalizeToken(tokenValue);
      credentialIntent.current += 1;
      let bootstrapCompleted = false;
      setBusy(true);
      setError(null);
      try {
        if (bootstrapFirst) {
          try {
            await api.bootstrap(token);
          } catch (bootstrapError) {
            let status: BootstrapStatus;
            try {
              status = await api.getBootstrapStatus();
            } catch {
              throw bootstrapError;
            }
            if (!isCurrentHost()) {
              return;
            }
            setBootstrapStatus(status);
            if (status.needsBootstrap) {
              throw bootstrapError;
            }
          }
          bootstrapCompleted = true;
          writeManagementSessionToken(token, storage, hostEpoch.namespace);
          if (isCurrentHost()) {
            setManagementToken(token);
          }
        }
        const verified = await api.verifySession(token);
        writeManagementSessionToken(token, storage, hostEpoch.namespace);
        if (!isCurrentHost()) {
          return;
        }
        setManagementToken(token);
        setSession(verified);
        setSecretGrant(null);
        setPhase("authenticated");
      } catch (cause) {
        const authenticationFailure = isAuthenticationError(cause);
        if (authenticationFailure) {
          clearLocalSession();
        }
        if (!isCurrentHost()) {
          throw cause;
        }
        if (!authenticationFailure) {
          if (bootstrapFirst && bootstrapCompleted) {
            setPhase("error");
          } else if (bootstrapFirst) {
            setPhase("bootstrap-required");
          } else {
            setPhase("unauthenticated");
          }
        }
        setError(errorMessage(cause));
        throw cause;
      } finally {
        if (isCurrentHost()) {
          setBusy(false);
        }
      }
    },
    [api, clearLocalSession, hostEpoch.namespace, isCurrentHost, storage],
  );

  const logout = useCallback(async () => {
    credentialIntent.current += 1;
    setBusy(true);
    setError(null);
    try {
      if (managementToken) {
        await api.logout(managementToken);
      }
    } catch (cause) {
      if (isCurrentHost()) {
        setError(errorMessage(cause));
      }
    } finally {
      clearLocalSession();
      if (isCurrentHost()) {
        setBusy(false);
      }
    }
  }, [api, clearLocalSession, isCurrentHost, managementToken]);

  const rotate = useCallback(
    async (newTokenValue: string) => {
      if (!managementToken) {
        throw new Error("No authenticated management session is available.");
      }
      const newToken = normalizeToken(newTokenValue);
      const intent = ++credentialIntent.current;
      const isCurrentRotation = () => isCurrentHost() && credentialIntent.current === intent;
      setBusy(true);
      setError(null);
      try {
        const verified = await rotateManagementToken(api, managementToken, newToken, isCurrentRotation);
        if (!isCurrentRotation()) return;
        writeManagementSessionToken(newToken, storage, hostEpoch.namespace);
        currentToken.current = newToken;
        setManagementToken(newToken);
        setSecretGrant(null);
        setSession((current) =>
          verified ? { ...verified, secretAccessGranted: false }
            : current ? { ...current, secretAccessGranted: false } : current,
        );
        setPhase("authenticated");
      } catch (cause) {
        if (!isCurrentRotation()) return;
        if (isAuthenticationError(cause)) {
          clearLocalSession();
        }
        if (isCurrentHost()) {
          setError(errorMessage(cause));
        }
        throw cause;
      } finally {
        if (isCurrentRotation()) {
          setBusy(false);
        }
      }
    },
    [
      api,
      clearLocalSession,
      hostEpoch.namespace,
      isCurrentHost,
      managementToken,
      storage,
    ],
  );

  const confirmSecretAccess = useCallback(
    async (confirmationTokenValue: string) => {
      if (!managementToken) {
        throw new Error("No authenticated management session is available.");
      }
      const confirmationToken = normalizeToken(confirmationTokenValue);
      const intent = credentialIntent.current;
      const isCurrentConfirmation = () => isCurrentHost() && credentialIntent.current === intent && currentToken.current === managementToken;
      setBusy(true);
      setError(null);
      try {
        const grant = await api.confirmSecretAccess(managementToken, confirmationToken);
        if (!isCurrentConfirmation()) {
          return;
        }
        setSecretGrant(grant);
        setSession((current) =>
          current ? { ...current, secretAccessGranted: true } : current,
        );
      } catch (cause) {
        if (!isCurrentConfirmation()) return;
        if (isAuthenticationError(cause)) {
          clearLocalSession();
        }
        if (isCurrentHost()) {
          setError(errorMessage(cause));
        }
        throw cause;
      } finally {
        if (isCurrentConfirmation()) {
          setBusy(false);
        }
      }
    },
    [api, clearLocalSession, isCurrentHost, managementToken],
  );

  const acceptSecretGrant = useCallback((grant: SecretGrant | null) => {
    setSecretGrant(grant);
    setSession((current) => current ? { ...current, secretAccessGranted: !!grant } : current);
  }, []);
  const secretBusy = useAutomaticSecretAccess({ api, token: managementToken, enabled: phase === "authenticated",
    grant: secretGrant, onGrant: acceptSecretGrant, onError: setError, onAuthenticationFailure: clearLocalSession });

  const value = useMemo<ManagementSessionContextValue>(
    () => ({
      phase,
      bootstrapStatus,
      session,
      managementToken,
      secretGrant,
      busy: busy || secretBusy,
      error,
      bootstrap: (token) => authenticateWith(token, true),
      login: (token) => authenticateWith(token, false),
      logout,
      rotate,
      confirmSecretAccess,
      clearSecretGrant: clearSecretAccess,
      retryInitialization: () => initialize(),
    }),
    [
      authenticateWith,
      bootstrapStatus,
      busy,
      confirmSecretAccess,
      clearSecretAccess,
      error,
      logout,
      managementToken,
      phase,
      rotate,
      secretGrant,
      secretBusy,
      session,
      initialize,
    ],
  );

  return (
    <ManagementSessionContext.Provider value={value}>
      {children}
    </ManagementSessionContext.Provider>
  );
}
