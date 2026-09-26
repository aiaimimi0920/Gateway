import {
  createContext,
  type PropsWithChildren,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

export type UiThemeMode = "system" | "dark" | "light";
export type UiThemeResolved = "dark" | "light";

type UiThemeContextValue = {
  mode: UiThemeMode;
  resolved: UiThemeResolved;
  setMode(mode: UiThemeMode): void;
  cycleMode(): void;
};

const STORAGE_KEY = "gateway-ui-theme";
const DEFAULT_MODE: UiThemeMode = "system";
const MODE_ORDER: readonly UiThemeMode[] = ["system", "dark", "light"];
const LIGHT_QUERY = "(prefers-color-scheme: light)";

function normalizeMode(value: string | null | undefined): UiThemeMode | null {
  if (value === "system" || value === "dark" || value === "light") {
    return value;
  }
  return null;
}

function lightQuery(): MediaQueryList | null {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return null;
  }
  return window.matchMedia(LIGHT_QUERY);
}

function detectInitialMode(): UiThemeMode {
  if (typeof window === "undefined") {
    return DEFAULT_MODE;
  }
  return normalizeMode(window.localStorage.getItem(STORAGE_KEY)) ?? DEFAULT_MODE;
}

function resolveTheme(mode: UiThemeMode, systemPrefersLight: boolean): UiThemeResolved {
  if (mode === "system") {
    return systemPrefersLight ? "light" : "dark";
  }
  return mode;
}

const defaultContextValue: UiThemeContextValue = {
  mode: DEFAULT_MODE,
  resolved: "dark",
  setMode: () => undefined,
  cycleMode: () => undefined,
};

const UiThemeContext = createContext<UiThemeContextValue>(defaultContextValue);

export function UiThemeProvider({ children }: PropsWithChildren) {
  const [mode, setMode] = useState<UiThemeMode>(() => detectInitialMode());
  const [systemPrefersLight, setSystemPrefersLight] = useState<boolean>(
    () => lightQuery()?.matches ?? false,
  );

  useEffect(() => {
    const query = lightQuery();
    if (!query || typeof query.addEventListener !== "function") {
      return;
    }
    const onChange = (event: MediaQueryListEvent) => setSystemPrefersLight(event.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  const resolved = resolveTheme(mode, systemPrefersLight);

  useEffect(() => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(STORAGE_KEY, mode);
    }
    if (typeof document !== "undefined") {
      document.documentElement.dataset.ntTheme = resolved;
      document.documentElement.dataset.ntThemeMode = mode;
    }
  }, [mode, resolved]);

  const cycleMode = useCallback(() => {
    setMode((current) => {
      const index = MODE_ORDER.indexOf(current);
      return MODE_ORDER[(index + 1) % MODE_ORDER.length];
    });
  }, []);

  const value = useMemo<UiThemeContextValue>(
    () => ({ mode, resolved, setMode, cycleMode }),
    [mode, resolved, cycleMode],
  );

  return <UiThemeContext.Provider value={value}>{children}</UiThemeContext.Provider>;
}

export function useUiTheme(): UiThemeContextValue {
  return useContext(UiThemeContext);
}
