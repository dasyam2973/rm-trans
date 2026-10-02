import { useMemo } from "react";
import { useProject } from "../stores/projectStore";
import type { Entry, GlossaryTerm, Kind } from "../types";
import { translationOf } from "./status";

export type Segment = { text: string; term?: GlossaryTerm };

/**
 * 단어장 매칭기. 일본어/중국어처럼 띄어쓰기가 없는 언어를 위해 단어 경계 없이 부분 문자열로 찾고,
 * 같은 위치에서는 긴 용어를 우선한다 ("ハロルド" 안의 "ハロ"는 따로 잡지 않음). AI 쪽(ai/glossary.rs)과 같은 원칙.
 */
export interface GlossaryMatcher {
  /** 번역이 있는 유효한 용어 수 */
  size: number;
  /** 텍스트를 용어/일반 구간으로 나눈다 */
  segment: (text: string) => Segment[];
  /** 텍스트에 등장하는 용어 (중복 없음) */
  termsIn: (text: string) => GlossaryTerm[];
}

export function buildGlossaryMatcher(glossary: GlossaryTerm[]): GlossaryMatcher {
  // 첫 글자별로 묶고 긴 것부터 정렬. 같은 원문은 앞의 것만 사용
  const byFirst = new Map<string, GlossaryTerm[]>();
  const seen = new Set<string>();
  for (const t of glossary) {
    if (!t.source || !t.target || seen.has(t.source)) continue;
    seen.add(t.source);
    const first = String.fromCodePoint(t.source.codePointAt(0)!);
    const list = byFirst.get(first) ?? [];
    list.push(t);
    byFirst.set(first, list);
  }
  for (const list of byFirst.values()) list.sort((a, b) => b.source.length - a.source.length);

  const segment = (text: string): Segment[] => {
    if (seen.size === 0) return [{ text }];
    const out: Segment[] = [];
    let plainStart = 0;
    let i = 0;
    while (i < text.length) {
      const first = String.fromCodePoint(text.codePointAt(i)!);
      const term = byFirst.get(first)?.find((t) => text.startsWith(t.source, i));
      if (term) {
        if (i > plainStart) out.push({ text: text.slice(plainStart, i) });
        out.push({ text: term.source, term });
        i += term.source.length;
        plainStart = i;
      } else {
        i += first.length;
      }
    }
    if (plainStart < text.length) out.push({ text: text.slice(plainStart) });
    return out;
  };

  const termsIn = (text: string) => {
    const found = new Set<GlossaryTerm>();
    for (const s of segment(text)) if (s.term) found.add(s.term);
    return [...found];
  };

  return { size: seen.size, segment, termsIn };
}

/**
 * 번역된 항목 중 원문에 단어장 용어가 있는데 번역문에 지정한 번역이 없는 용어들.
 * 미번역 항목은 검사하지 않는다 (빈 배열).
 */
export function glossaryIssues(entry: Entry, translations: Record<string, string>, matcher: GlossaryMatcher): GlossaryTerm[] {
  if (matcher.size === 0) return [];
  const translation = translations[entry.id];
  if (translation === undefined) return [];
  return matcher.termsIn(entry.original).filter((t) => !translation.includes(t.target));
}

export interface GlossaryCandidate {
  source: string;
  /** 이미 번역했다면 그 번역 */
  target: string;
  category: string;
}

/** 단어장 후보로 쓸 만한 항목 종류: (종류, 파일 이름) → 분류 */
function candidateCategory(entry: Entry): string | null {
  const base = entry.file.slice(entry.file.lastIndexOf("/") + 1);
  const kind: Kind = entry.kind;
  if (kind === "speaker") return "화자 이름";
  if (kind === "displayName") return "맵 표시명";
  if (base === "Actors.json") return kind === "name" ? "액터 이름" : kind === "nickname" ? "액터 닉네임" : null;
  if (kind !== "name") return null;
  switch (base) {
    case "Classes.json":
      return "직업";
    case "Enemies.json":
      return "적";
    case "Skills.json":
      return "스킬";
    case "Items.json":
    case "Weapons.json":
    case "Armors.json":
      return "아이템·장비";
    case "States.json":
      return "상태";
    default:
      return null;
  }
}

export const CANDIDATE_CATEGORIES = ["액터 이름", "액터 닉네임", "화자 이름", "맵 표시명", "직업", "적", "스킬", "아이템·장비", "상태"];

/** 이름류 항목에서 단어장 후보를 모은다. 이미 단어장에 있는 원문과 빈 문자열은 제외하고, 같은 원문은 처음 것만 남긴다. */
export function collectCandidates(
  entries: Entry[],
  translations: Record<string, string>,
  existing: Set<string>,
): GlossaryCandidate[] {
  const seen = new Set(existing);
  const out: GlossaryCandidate[] = [];
  for (const e of entries) {
    const source = e.original.trim();
    if (!source || seen.has(source)) continue;
    const category = candidateCategory(e);
    if (!category) continue;
    seen.add(source);
    const t = translationOf(e, translations).trim();
    out.push({ source, target: t === source ? "" : t, category });
  }
  return out;
}

/** 현재 프로젝트 단어장의 매칭기 (단어장이 바뀔 때만 다시 만든다) */
export function useGlossaryMatcher(): GlossaryMatcher {
  const glossary = useProject((s) => s.glossary);
  return useMemo(() => buildGlossaryMatcher(glossary), [glossary]);
}
