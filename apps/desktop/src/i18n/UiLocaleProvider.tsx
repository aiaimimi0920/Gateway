import {
  createContext,
  type PropsWithChildren,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

export type UiLocale = "zh-CN" | "en-US";

type UiLocaleContextValue = {
  locale: UiLocale;
  setLocale(locale: UiLocale): void;
  toggleLocale(): void;
  t(zh: string, en: string): string;
};

const STORAGE_KEY = "gateway-ui-locale";
const DEFAULT_LOCALE: UiLocale = "zh-CN";

function normalizeLocale(value: string | null | undefined): UiLocale | null {
  if (value === "zh-CN" || value === "en-US") {
    return value;
  }
  return null;
}

function detectInitialLocale(): UiLocale {
  if (typeof window === "undefined") {
    return DEFAULT_LOCALE;
  }
  const stored = normalizeLocale(window.localStorage.getItem(STORAGE_KEY));
  if (stored) {
    return stored;
  }
  return DEFAULT_LOCALE;
}

const defaultContextValue: UiLocaleContextValue = {
  locale: DEFAULT_LOCALE,
  setLocale: () => undefined,
  toggleLocale: () => undefined,
  t: (zh) => zh,
};

const UiLocaleContext = createContext<UiLocaleContextValue>(defaultContextValue);

export function UiLocaleProvider({ children }: PropsWithChildren) {
  const [locale, setLocale] = useState<UiLocale>(() => detectInitialLocale());

  useEffect(() => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(STORAGE_KEY, locale);
    }
    if (typeof document !== "undefined") {
      document.documentElement.lang = locale;
    }
  }, [locale]);

  const toggleLocale = useCallback(() => {
    setLocale((current) => (current === "zh-CN" ? "en-US" : "zh-CN"));
  }, []);

  const t = useCallback(
    (zh: string, en: string) => (locale === "zh-CN" ? zh : en),
    [locale],
  );

  const value = useMemo<UiLocaleContextValue>(
    () => ({
      locale,
      setLocale,
      toggleLocale,
      t,
    }),
    [locale, t],
  );

  return <UiLocaleContext.Provider value={value}>{children}</UiLocaleContext.Provider>;
}

export function useUiLocale(): UiLocaleContextValue {
  return useContext(UiLocaleContext);
}
