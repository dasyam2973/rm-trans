import type { Entry, Kind, Status } from "../types";
import { glossaryIssues, type GlossaryMatcher } from "./glossary";
import { statusOf, translationOf } from "./status";

/** context: 맥락(DB 필드 이름·화자)과 그룹 이름 (WOLF RPG DB 대사처럼 원문만으로는 고르기 어려운 항목용) */
export type SearchScope = "both" | "original" | "translation" | "path" | "context";

/** 경로 검색용: JSON Pointer 토큰 이스케이프(~1 → /, ~0 → ~)를 풀어 표시되는 키 이름 그대로 찾을 수 있게 한다 */
const searchablePath = (path: string) => path.replace(/~1/g, "/").replace(/~0/g, "~");
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
  /** 선택한 항목만 보기: 켠 순간의 선택을 고정해 둔 ID 집합 (null이면 꺼짐). 이후 선택을 바꿔도 목록은 그대로 */
  pinnedIds: Set<string> | null;
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
    if (opts.pinnedIds && !opts.pinnedIds.has(e.id)) return false;
    if (files && !files.has(e.file)) return false;
    if (kinds && !kinds.has(e.kind)) return false;
    if (opts.status !== "all" && statusOf(e, translations, overrides) !== opts.status) return false;
    // 단어장이 비면 토글 버튼이 숨겨지므로 필터도 무시한다
    if (opts.glossaryIssues && glossary.size > 0 && glossaryIssues(e, translations, glossary).length === 0) return false;
    if (matcher?.test) {
      const hit =
        opts.scope === "path"
          ? matcher.test(searchablePath(e.path))
          : opts.scope === "context"
            ? matcher.test(e.context ?? "") || matcher.test(e.groupLabel)
            : (opts.scope !== "translation" && matcher.test(e.original)) ||
              (opts.scope !== "original" && matcher.test(translationOf(e, translations)));
      if (!hit) return false;
    }
    return true;
  });
  return { result };
}
