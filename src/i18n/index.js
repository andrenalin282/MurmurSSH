import en from "./en";
import de from "./de";
const locales = { en, de };
const STORAGE_KEY = "murmurssh_locale";
let currentLocaleKey = "en";
function loadLocale() {
    try {
        const stored = localStorage.getItem(STORAGE_KEY);
        if (stored && stored in locales) {
            currentLocaleKey = stored;
        }
    }
    catch {
        // localStorage not available — use default
    }
}
export function setLocale(key) {
    if (key in locales) {
        currentLocaleKey = key;
        try {
            localStorage.setItem(STORAGE_KEY, key);
        }
        catch {
            // Non-fatal
        }
    }
}
export function getLocale() {
    return currentLocaleKey;
}
export function getAvailableLocales() {
    return [
        { key: "en", label: "English" },
        { key: "de", label: "Deutsch" },
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
export function t(key, vars) {
    const locale = locales[currentLocaleKey] ?? en;
    const parts = key.split(".");
    let value = locale;
    for (const part of parts) {
        if (typeof value !== "object" || value === null)
            return key;
        value = value[part];
    }
    if (typeof value !== "string")
        return key;
    if (!vars)
        return value;
    return value.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? `{${k}}`));
}
// Initialize on module load
loadLocale();
