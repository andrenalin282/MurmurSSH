import en from "./en";
import de from "./de";
import fr from "./fr";
import nl from "./nl";
import ru from "./ru";
import pl from "./pl";
type DeepStringify<T> = T extends string ? string : { [K in keyof T]: DeepStringify<T[K]> };
type Locale = DeepStringify<typeof en>;

const locales: Record<string, Locale> = { en, de, fr, nl, ru, pl };
const STORAGE_KEY = "murmurssh_locale";

let currentLocaleKey: string = "en";

function loadLocale(): void {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored && stored in locales) {
      currentLocaleKey = stored;
    }
  } catch {
    // localStorage not available — use default
  }
}

export function setLocale(key: string): void {
  if (key in locales) {
    currentLocaleKey = key;
    try {
      localStorage.setItem(STORAGE_KEY, key);
    } catch {
      // Non-fatal
    }
  }
}

export function getLocale(): string {
  return currentLocaleKey;
}

export function getAvailableLocales(): Array<{ key: string; label: string }> {
  return [
    { key: "en", label: "English" },
    { key: "de", label: "Deutsch" },
    { key: "fr", label: "Français" },
    { key: "nl", label: "Nederlands" },
    { key: "ru", label: "Русский" },
    { key: "pl", label: "Polski" },
  ];
}

/**
 * Translate a dot-separated key, substituting {varName} placeholders if vars are provided.
 * Falls back to the key string itself if the key is not found.
 *
 * Example:
 *   t("common.cancel")                          → "Cancel"
 *   t("connection.connectedTo", { host: "srv" }) → "Connected to srv"
 */
export function t(key: string, vars?: Record<string, string | number>): string {
  const locale: Locale = (locales[currentLocaleKey] as Locale) ?? en;
  const parts = key.split(".");
  let value: unknown = locale;
  for (const part of parts) {
    if (typeof value !== "object" || value === null) return key;
    value = (value as Record<string, unknown>)[part];
  }
  if (typeof value !== "string") return key;
  if (!vars) return value;
  return value.replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? `{${k}}`));
}

// Initialize on module load
loadLocale();
