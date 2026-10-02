import { useMemo } from "react";
import { KIND_LABELS, type Entry, type Kind } from "../../types";
import type { SearchScope, StatusFilter } from "../../lib/filter";
import { glossaryIssues, useGlossaryMatcher } from "../../lib/glossary";
import { useFilter } from "../../stores/filterStore";
import { useProject } from "../../stores/projectStore";
import { useSelection } from "../../stores/selectionStore";
import { Button, Toggle, inputClass } from "../ui";

export function FilterBar({ filtered, total, error }: { filtered: Entry[]; total: number; error?: string }) {
  const f = useFilter();
  const { setMany, clear, selected } = useSelection();
  const entries = useProject((s) => s.entries);
  const presentKinds = useMemo(() => {
    const present = new Set(entries.map((e) => e.kind));
    return (Object.keys(KIND_LABELS) as Kind[]).filter((k) => present.has(k));
  }, [entries]);
  const translations = useProject((s) => s.translations);
  const glossary = useGlossaryMatcher();
  const issueCount = useMemo(
    () => (glossary.size ? entries.filter((e) => glossaryIssues(e, translations, glossary).length > 0).length : 0),
    [entries, translations, glossary],
  );
  const toggleKind = (k: Kind) =>
    f.set({ kinds: f.kinds.includes(k) ? f.kinds.filter((x) => x !== k) : [...f.kinds, k] });

  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-zinc-800 bg-zinc-900/60 px-3 py-2">
      <div className="flex items-center gap-1">
        <input
          className={`${inputClass} w-72 ${error ? "border-rose-500" : ""}`}
          placeholder={f.regex ? "정규식 검색" : "검색"}
          value={f.query}
          onChange={(e) => f.set({ query: e.target.value })}
          title={error}
        />
        <Toggle on={f.regex} onChange={(regex) => f.set({ regex })} title="정규식">
          .*
        </Toggle>
        <Toggle on={f.caseSensitive} onChange={(caseSensitive) => f.set({ caseSensitive })} title="대소문자 구분">
          Aa
        </Toggle>
      </div>

      <select className={inputClass} value={f.scope} onChange={(e) => f.set({ scope: e.target.value as SearchScope })}>
        <option value="both">원문 + 번역문</option>
        <option value="original">원문만</option>
        <option value="translation">번역문만</option>
      </select>

      <div className="flex overflow-hidden rounded border border-zinc-700">
        {(
          [
            ["all", "전체"],
            ["untranslated", "미번역"],
            ["translated", "번역"],
          ] as [StatusFilter, string][]
        ).map(([v, label]) => (
          <button
            key={v}
            onClick={() => f.set({ status: v })}
            className={`px-2.5 py-1 text-sm ${f.status === v ? "bg-sky-600/40 text-sky-100" : "text-zinc-400 hover:bg-zinc-800"}`}
          >
            {label}
          </button>
        ))}
      </div>

      {glossary.size > 0 && (
        <button
          onClick={() => f.set({ glossaryIssues: !f.glossaryIssues })}
          title="번역된 항목 중 원문에 단어장 용어가 있는데 번역문에 지정한 번역이 없는 항목만 표시"
          className={`rounded border px-2 py-1 text-sm ${
            f.glossaryIssues
              ? "border-amber-500 bg-amber-600/30 text-amber-100"
              : "border-zinc-700 text-zinc-400 hover:bg-zinc-800"
          }`}
        >
          단어장 불일치 {issueCount.toLocaleString()}
        </button>
      )}

      {presentKinds.length > 1 && (
        <div className="flex flex-wrap items-center gap-1" title="종류별 필터 (여러 개 선택 가능, 선택 없음 = 전체)">
          {presentKinds.map((k) => (
            <button
              key={k}
              onClick={() => toggleKind(k)}
              className={`rounded border px-2 py-0.5 text-xs ${
                f.kinds.includes(k)
                  ? "border-sky-500 bg-sky-600/40 text-sky-100"
                  : "border-zinc-700 text-zinc-400 hover:bg-zinc-800"
              }`}
            >
              {KIND_LABELS[k]}
            </button>
          ))}
          {f.kinds.length > 0 && (
            <button onClick={() => f.set({ kinds: [] })} className="px-1 text-xs text-zinc-500 hover:text-zinc-300">
              ✕
            </button>
          )}
        </div>
      )}

      {error && <span className="text-xs text-rose-400">정규식 오류</span>}

      <span className="ml-auto text-xs text-zinc-400">
        {filtered.length.toLocaleString()} / {total.toLocaleString()}개
      </span>
      <Button variant="ghost" onClick={() => setMany(filtered.map((e) => e.id), true)} disabled={filtered.length === 0}>
        결과 전체 선택
      </Button>
      <Button variant="ghost" onClick={clear} disabled={selected.size === 0}>
        선택 해제
      </Button>
    </div>
  );
}
