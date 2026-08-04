import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./en.json";
import it from "./it.json";

export type AppLanguage = "it" | "en";

export function systemLanguage(): AppLanguage {
  return typeof navigator !== "undefined" &&
    navigator.language?.toLowerCase().startsWith("it")
    ? "it"
    : "en";
}

/** "system" | "it" | "en" (from settings) → concrete language. */
export function resolveLanguage(pref: string): AppLanguage {
  return pref === "it" || pref === "en" ? pref : systemLanguage();
}

i18n.use(initReactI18next).init({
  resources: {
    it: { translation: it },
    en: { translation: en },
  },
  lng: systemLanguage(),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
  react: { useSuspense: false },
});

export default i18n;
