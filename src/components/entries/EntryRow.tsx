import { memo, useEffect, useState, type MouseEvent } from "react";
import { KIND_LABELS, type Entry, type Status } from "../../types";

interface Props {
  entry: Entry;
  translation: string;
  status: Status;
  override: Status | undefined;
  selected: boolean;
  onSelect: (id: string, e: MouseEvent) => void;
  onCommit: (id: string, text: string) => void;
  onOverride: (id: string, status: Status | null) => void;
}

export const EntryRow = memo(function EntryRow({
  entry,
  translation,
  status,
  override,
  selected,
  onSelect,
  onCommit,
  onOverride,
}: Props) {
  // 입력 중에는 로컬 상태만 바꾸고, 포커스가 빠질 때 스토어에 반영한다.
  // (매 입력마다 반영하면 필터 결과가 바뀌어 편집 중인 행이 사라질 수 있음)
  const [draft, setDraft] = useState(translation);
  useEffect(() => setDraft(translation), [translation]);

  const commit = () => {
    if (draft !== translation) onCommit(entry.id, draft);
  };

  return (
    <div
      className={`grid grid-cols-[28px_1fr_1fr_92px] gap-2 border-b border-zinc-800/70 py-1.5 pr-3 pl-5 ${
        selected ? "bg-sky-900/25" : "hover:bg-zinc-900/60"
      }`}
    >
      <div className="pt-5">
        <input
          type="checkbox"
          checked={selected}
          onChange={() => {}}
          onClick={(e) => onSelect(entry.id, e)}
          className="accent-sky-500"
        />
      </div>

      <div className="min-w-0">
        <div className="mb-0.5 flex items-center gap-1.5 text-[11px] text-zinc-500">
          <span className="rounded bg-zinc-800 px-1 text-zinc-400">{KIND_LABELS[entry.kind]}</span>
          {entry.context && <span className="text-amber-300/80">{entry.context}</span>}
          <span className="truncate font-mono" title={entry.id}>
            {entry.path}
          </span>
        </div>
        <div className="rounded bg-zinc-900/60 px-2 py-1 whitespace-pre-wrap break-words text-zinc-300 select-text">
          {entry.original}
        </div>
      </div>

      <div className="min-w-0 pt-[18px]">
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Escape") setDraft(translation);
          }}
          rows={1}
          spellCheck={false}
          className={`w-full resize-none rounded border bg-zinc-900 px-2 py-1 text-zinc-100 outline-none [field-sizing:content] focus:border-sky-500 ${
            draft !== translation ? "border-amber-500/70" : "border-zinc-700"
          }`}
        />
      </div>

      <div className="flex flex-col items-stretch gap-1 pt-[18px]">
        <span
          className={`rounded px-1.5 py-0.5 text-center text-xs ${
            status === "translated" ? "bg-emerald-600/25 text-emerald-300" : "bg-zinc-800 text-zinc-400"
          }`}
        >
          {status === "translated" ? "번역" : "미번역"}
        </span>
        <select
          value={override ?? "auto"}
          onChange={(e) => onOverride(entry.id, e.target.value === "auto" ? null : (e.target.value as Status))}
          className="rounded border border-zinc-800 bg-zinc-900 px-1 py-0.5 text-xs text-zinc-400"
          title="상태 지정 방식"
        >
          <option value="auto">자동</option>
          <option value="translated">번역으로 지정</option>
          <option value="untranslated">미번역으로 지정</option>
        </select>
      </div>
    </div>
  );
});
