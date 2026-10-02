import { useMemo, useRef, useState } from "react";
import { CANDIDATE_CATEGORIES, collectCandidates } from "../../lib/glossary";
import { useProject } from "../../stores/projectStore";
import type { GlossaryTerm } from "../../types";
import { Button, Dialog, inputClass } from "../ui";

interface Row extends Required<GlossaryTerm> {
  key: number;
}

let nextKey = 0;
const toRow = (t: GlossaryTerm): Row => ({ key: nextKey++, source: t.source, target: t.target, note: t.note ?? "" });

/** 인명/고유명사 단어장 편집. 적용을 눌러야 작업 상태에 반영된다. */
export function GlossaryDialog({ onClose }: { onClose: () => void }) {
  const initial = useMemo(() => useProject.getState().glossary.map(toRow), []);
  const [rows, setRows] = useState<Row[]>(initial);
  const [changed, setChanged] = useState(false);
  const [search, setSearch] = useState("");
  const [pasteOpen, setPasteOpen] = useState(false);
  const [candidatesOpen, setCandidatesOpen] = useState(false);
  const [paste, setPaste] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const firstInput = useRef<HTMLInputElement>(null);

  const update = (next: Row[]) => {
    setRows(next);
    setChanged(true);
    setMessage(null);
  };
  const edit = (key: number, field: keyof GlossaryTerm, value: string) =>
    update(rows.map((r) => (r.key === key ? { ...r, [field]: value } : r)));

  const duplicates = useMemo(() => {
    const count = new Map<string, number>();
    for (const r of rows) {
      const s = r.source.trim();
      if (s) count.set(s, (count.get(s) ?? 0) + 1);
    }
    return new Set([...count].filter(([, n]) => n > 1).map(([s]) => s));
  }, [rows]);

  const visible = useMemo(() => {
    const q = search.trim();
    if (!q) return rows;
    return rows.filter((r) => r.source.includes(q) || r.target.includes(q) || r.note.includes(q));
  }, [rows, search]);

  const addRow = () => {
    setSearch("");
    update([toRow({ source: "", target: "" }), ...rows]);
    // 새 행이 렌더링된 뒤 포커스
    setTimeout(() => firstInput.current?.focus());
  };

  /** "원문<탭>번역<탭>메모" 줄들을 추가한다. 이미 있는 원문은 번역/메모를 덮어쓴다. */
  const addPasted = () => {
    const next = rows.map((r) => ({ ...r }));
    let added = 0;
    let updated = 0;
    for (const line of paste.split(/\r?\n/)) {
      const [source = "", target = "", note = ""] = line.split("\t").map((s) => s.trim());
      if (!source) continue;
      const existing = next.find((r) => r.source.trim() === source);
      if (existing) {
        Object.assign(existing, { target, note });
        updated++;
      } else {
        next.push(toRow({ source, target, note }));
        added++;
      }
    }
    update(next);
    setPaste("");
    setPasteOpen(false);
    setMessage(`${added}개 추가, ${updated}개 갱신했습니다.`);
  };

  const apply = () => {
    const glossary: GlossaryTerm[] = rows
      .map((r) => ({ source: r.source.trim(), target: r.target.trim(), note: r.note.trim() }))
      .filter((t) => t.source || t.target)
      .map(({ note, ...t }) => (note ? { ...t, note } : t));
    useProject.getState().setGlossary(glossary);
    onClose();
  };

  const close = () => {
    if (changed && !confirm("적용하지 않은 변경 사항이 있습니다. 닫을까요?")) return;
    onClose();
  };

  const missingTarget = rows.filter((r) => r.source.trim() && !r.target.trim()).length;

  return (
    <Dialog
      title="단어장"
      width="w-[760px]"
      onClose={close}
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            취소
          </Button>
          <Button variant="primary" onClick={apply} disabled={!changed || duplicates.size > 0}>
            적용
          </Button>
        </>
      }
    >
      <p className="mb-3 text-xs text-zinc-500">
        인명·지명 등 고정된 번역을 등록하면, AI 번역 시 원문에 등장하는 용어만 골라 함께 보냅니다. 원문은 띄어쓰기 없이
        부분 문자열로 찾으며, 겹치는 경우 긴 용어가 우선합니다. 적용한 단어장은 작업 저장 시 함께 저장됩니다.
      </p>

      <div className="mb-3 flex gap-2">
        <input
          className={`${inputClass} flex-1`}
          placeholder="검색"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <Button onClick={addRow}>+ 항목 추가</Button>
        <Button
          onClick={() => {
            setPasteOpen(!pasteOpen);
            setCandidatesOpen(false);
          }}
        >
          붙여넣기로 추가
        </Button>
        <Button
          onClick={() => {
            setCandidatesOpen(!candidatesOpen);
            setPasteOpen(false);
          }}
        >
          후보 가져오기
        </Button>
      </div>

      {candidatesOpen && (
        <CandidatePanel
          existing={rows}
          onAdd={(terms) => {
            update([...terms.map(toRow), ...rows]);
            setCandidatesOpen(false);
            setMessage(`${terms.length}개 추가했습니다.`);
          }}
        />
      )}

      {pasteOpen && (
        <div className="mb-3 rounded border border-zinc-700 p-2">
          <p className="mb-1 text-xs text-zinc-400">
            한 줄에 하나씩 <code>원문⇥번역⇥메모</code> 형식으로 붙여넣으세요 (탭으로 구분, 메모는 생략 가능). 스프레드시트에서
            복사하면 이 형식이 됩니다. 이미 있는 원문은 덮어씁니다.
          </p>
          <textarea
            className={`${inputClass} h-28 w-full font-mono`}
            value={paste}
            onChange={(e) => setPaste(e.target.value)}
          />
          <div className="mt-1 flex justify-end">
            <Button onClick={addPasted} disabled={!paste.trim()}>
              추가
            </Button>
          </div>
        </div>
      )}

      {rows.length === 0 ? (
        <p className="py-8 text-center text-sm text-zinc-500">등록된 용어가 없습니다.</p>
      ) : (
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-xs text-zinc-400">
              <th className="pb-1 font-normal">원문</th>
              <th className="pb-1 font-normal">번역</th>
              <th className="pb-1 font-normal">메모</th>
              <th className="w-8" />
            </tr>
          </thead>
          <tbody>
            {visible.map((r, i) => (
              <tr key={r.key}>
                <td className="pr-1 pb-1">
                  <input
                    ref={i === 0 ? firstInput : undefined}
                    className={`${inputClass} w-full ${duplicates.has(r.source.trim()) ? "border-rose-500!" : ""}`}
                    value={r.source}
                    onChange={(e) => edit(r.key, "source", e.target.value)}
                  />
                </td>
                <td className="pr-1 pb-1">
                  <input
                    className={`${inputClass} w-full`}
                    value={r.target}
                    onChange={(e) => edit(r.key, "target", e.target.value)}
                  />
                </td>
                <td className="pr-1 pb-1">
                  <input
                    className={`${inputClass} w-full`}
                    placeholder="성별, 말투 등"
                    value={r.note}
                    onChange={(e) => edit(r.key, "note", e.target.value)}
                  />
                </td>
                <td className="pb-1 text-center">
                  <button
                    className="text-zinc-500 hover:text-rose-400"
                    title="삭제"
                    onClick={() => update(rows.filter((x) => x.key !== r.key))}
                  >
                    ✕
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <div className="mt-2 space-y-0.5 text-xs">
        <p className="text-zinc-500">
          {rows.length.toLocaleString()}개
          {search.trim() && ` 중 ${visible.length.toLocaleString()}개 표시`}
        </p>
        {duplicates.size > 0 && <p className="text-rose-400">원문이 중복된 항목이 있습니다: {[...duplicates].join(", ")}</p>}
        {missingTarget > 0 && <p className="text-amber-400">번역이 비어 있는 {missingTarget}개는 AI 번역에 사용되지 않습니다.</p>}
        {message && <p className="text-emerald-400">{message}</p>}
      </div>
    </Dialog>
  );
}

/** 액터·화자·맵 이름 등 이름류 항목에서 단어장 후보를 골라 추가한다. 이미 번역한 항목은 그 번역을 채워 넣는다. */
function CandidatePanel({ existing, onAdd }: { existing: Row[]; onAdd: (terms: GlossaryTerm[]) => void }) {
  const { entries, translations } = useProject();
  const candidates = useMemo(
    () => collectCandidates(entries, translations, new Set(existing.map((r) => r.source.trim()).filter(Boolean))),
    [entries, translations, existing],
  );
  const categories = useMemo(
    () =>
      CANDIDATE_CATEGORIES.map((c) => [c, candidates.filter((x) => x.category === c).length] as const).filter(
        ([, n]) => n > 0,
      ),
    [candidates],
  );
  const [category, setCategory] = useState<string | null>(null);
  const current = category ?? categories[0]?.[0] ?? null;
  const list = candidates.filter((c) => c.category === current);
  const [checked, setChecked] = useState<Set<string>>(new Set());

  const toggle = (source: string) => {
    const next = new Set(checked);
    if (next.has(source)) next.delete(source);
    else next.add(source);
    setChecked(next);
  };
  const allChecked = list.length > 0 && list.every((c) => checked.has(c.source));
  const toggleAll = () => {
    const next = new Set(checked);
    for (const c of list) {
      if (allChecked) next.delete(c.source);
      else next.add(c.source);
    }
    setChecked(next);
  };

  return (
    <div className="mb-3 rounded border border-zinc-700 p-2">
      {categories.length === 0 ? (
        <p className="py-2 text-center text-xs text-zinc-500">추가할 만한 후보가 없습니다.</p>
      ) : (
        <>
          <div className="mb-2 flex flex-wrap gap-1">
            {categories.map(([c, n]) => (
              <button
                key={c}
                onClick={() => setCategory(c)}
                className={`rounded border px-2 py-0.5 text-xs ${
                  c === current ? "border-sky-500 bg-sky-600/40 text-sky-100" : "border-zinc-700 text-zinc-400 hover:bg-zinc-800"
                }`}
              >
                {c} {n}
              </button>
            ))}
          </div>
          <label className="mb-1 flex items-center gap-2 text-xs text-zinc-400">
            <input type="checkbox" className="accent-sky-500" checked={allChecked} onChange={toggleAll} />
            모두 선택
          </label>
          <div className="max-h-48 overflow-y-auto">
            {list.map((c) => (
              <label key={c.source} className="flex items-center gap-2 py-0.5 text-sm">
                <input
                  type="checkbox"
                  className="accent-sky-500"
                  checked={checked.has(c.source)}
                  onChange={() => toggle(c.source)}
                />
                <span>{c.source}</span>
                {c.target ? (
                  <span className="text-zinc-400">→ {c.target}</span>
                ) : (
                  <span className="text-xs text-zinc-600">(미번역)</span>
                )}
              </label>
            ))}
          </div>
          <div className="mt-2 flex items-center justify-end gap-2">
            <span className="text-xs text-zinc-500">미번역 후보는 번역을 비운 채 추가됩니다.</span>
            <Button
              disabled={checked.size === 0}
              onClick={() =>
                onAdd(candidates.filter((c) => checked.has(c.source)).map(({ source, target }) => ({ source, target })))
              }
            >
              선택 항목 추가 ({checked.size})
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
