import type { Entry, Kind, Status } from "../types";
import { glossaryIssues, type GlossaryMatcher } from "./glossary";
import { statusOf, translationOf } from "./status";

export type SearchScope = "both" | "original" | "translation";
export type StatusFilter = "all" | Status;

export interface FilterOptions {
  query: string;
  regex: boolean;
  caseSensitive: boolean;
  scope: SearchScope;
  status: StatusFilter;
  /** 비어 있으면 전체 파일 */
  files: string[];
  /** 비어 있으면 전체 종류 */
  kinds: Kind[];
  /** 단어장 용어가 지정한 번역대로 번역되지 않은 항목만 */
  glossaryIssues: boolean;
}

export type Matcher = { test: (s: string) => boolean; error?: undefined } | { test?: undefined; error: string };

export function buildMatcher(query: string, regex: boolean, caseSensitive: boolean): Matcher {
  if (regex) {
    try {
      const re = new RegExp(query, caseSensitive ? "u" : "iu");
      return { test: (s) => re.test(s) };
    } catch (e) {
      return { error: (e as Error).message };
    }
  }
  if (caseSensitive) return { test: (s) => s.includes(query) };
  const q = query.toLowerCase();
  return { test: (s) => s.toLowerCase().includes(q) };
}

export function filterEntries(
  entries: Entry[],
  translations: Record<string, string>,
  overrides: Record<string, Status>,
  opts: FilterOptions,
  glossary: GlossaryMatcher,
): { result: Entry[]; error?: string } {
  const matcher = opts.query ? buildMatcher(opts.query, opts.regex, opts.caseSensitive) : null;
  if (matcher?.error) return { result: entries, error: matcher.error };
  const files = opts.files.length ? new Set(opts.files) : null;
  const kinds = opts.kinds.length ? new Set(opts.kinds) : null;

  const result = entries.filter((e) => {
    if (files && !files.has(e.file)) return false;
    if (kinds && !kinds.has(e.kind)) return false;
    if (opts.status !== "all" && statusOf(e, translations, overrides) !== opts.status) return false;
    // 단어장이 비면 토글 버튼이 숨겨지므로 필터도 무시한다
    if (opts.glossaryIssues && glossary.size > 0 && glossaryIssues(e, translations, glossary).length === 0) return false;
    if (matcher?.test) {
      const t = translationOf(e, translations);
      const hit =
        (opts.scope !== "translation" && matcher.test(e.original)) ||
        (opts.scope !== "original" && matcher.test(t));
      if (!hit) return false;
    }
    return true;
  });
  return { result };
}
