import type { Entry, Status } from "../types";

/** 번역문이 따로 없으면 원문을 그대로 번역문으로 본다. */
export function translationOf(entry: Entry, translations: Record<string, string>): string {
  return translations[entry.id] ?? entry.original;
}

/** 사용자가 지정한 상태가 있으면 그것을, 없으면 원문과 번역문이 다른지로 판정한다. */
export function statusOf(
  entry: Entry,
  translations: Record<string, string>,
  overrides: Record<string, Status>,
): Status {
  return overrides[entry.id] ?? (translationOf(entry, translations) !== entry.original ? "translated" : "untranslated");
}
