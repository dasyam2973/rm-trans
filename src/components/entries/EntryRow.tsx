import { memo, useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import type { GlossaryMatcher } from "../../lib/glossary";
import { isRiskyKind, KIND_LABELS, type Entry, type GlossaryTerm, type Status } from "../../types";

interface Props {
  entry: Entry;
  translation: string;
  status: Status;
  override: Status | undefined;
  selected: boolean;
  glossary: GlossaryMatcher;
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
  glossary,
  onSelect,
  onCommit,
  onOverride,
}: Props) {
  // 입력 중에는 로컬 상태만 바꾸고, 포커스가 빠질 때 스토어에 반영한다.
  // (매 입력마다 반영하면 필터 결과가 바뀌어 편집 중인 행이 사라질 수 있음)
  const [draft, setDraft] = useState(translation);
  useEffect(() => setDraft(translation), [translation]);
  const textarea = useRef<HTMLTextAreaElement>(null);
  /** 한 번이라도 입력란에 포커스했는지. 아니면 커서 위치가 의미 없으므로 끝에 삽입한다 */
  const touched = useRef(false);

  const commit = () => {
    if (draft !== translation) onCommit(entry.id, draft);
  };

  const segments = useMemo(() => glossary.segment(entry.original), [glossary, entry.original]);
  // 번역된 항목에서만 검사 (미번역이면 당연히 없으므로)
  const issues = useMemo(
    () => (status === "translated" ? glossary.termsIn(entry.original).filter((t) => !draft.includes(t.target)) : []),
    [glossary, entry.original, draft, status],
  );

  /** 용어 번역을 입력란의 커서 위치(처음 편집이면 끝)에 넣는다. 반영은 평소처럼 포커스가 빠질 때 된다. */
  const insertTerm = (term: GlossaryTerm) => {
    const el = textarea.current;
    if (!el) return;
    const [start, end] = touched.current ? [el.selectionStart, el.selectionEnd] : [draft.length, draft.length];
    const next = draft.slice(0, start) + term.target + draft.slice(end);
    setDraft(next);
    const caret = start + term.target.length;
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(caret, caret);
    });
  };

  const [replaceHint, setReplaceHint] = useState(false);
  /**
   * 불일치 용어를 지정 번역으로 치환한다. 번역문에 원문 용어가 그대로 남아 있으면 전부 바꾸고,
   * 아니면 입력란에서 선택한 부분을 바꾼다. 잘못 번역된 표현은 자동으로 알 수 없으므로 선택이 없으면 안내만 한다.
   */
  const replaceTerm = (term: GlossaryTerm) => {
    const el = textarea.current;
    if (!el) return;
    if (draft.includes(term.source)) {
      setDraft(draft.split(term.source).join(term.target));
      setReplaceHint(false);
      requestAnimationFrame(() => el.focus());
      return;
    }
    const [start, end] = [el.selectionStart, el.selectionEnd];
    if (!touched.current || start === end) {
      setReplaceHint(true);
      el.focus();
      return;
    }
    setDraft(draft.slice(0, start) + term.target + draft.slice(end));
    setReplaceHint(false);
    const caret = start + term.target.length;
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(caret, caret);
    });
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
          {isRiskyKind(entry.kind) ? (
            <span
              className="rounded bg-amber-600/25 px-1 text-amber-300"
              title="플러그인·외부 JSON 데이터입니다. 화면에 표시되는 문구인지 확인한 뒤 번역하세요. 파일명·식별자·스크립트 등을 바꾸면 게임이 깨질 수 있습니다."
            >
              ⚠ {KIND_LABELS[entry.kind]}
            </span>
          ) : (
            <span className="rounded bg-zinc-800 px-1 text-zinc-400">{KIND_LABELS[entry.kind]}</span>
          )}
          {entry.context && <span className="text-amber-300/80">{entry.context}</span>}
          <span className="truncate font-mono" title={entry.id}>
            {entry.path}
          </span>
        </div>
        <div className="rounded bg-zinc-900/60 px-2 py-1 whitespace-pre-wrap break-words text-zinc-300 select-text">
          {segments.map((s, i) =>
            s.term ? (
              <span
                key={i}
                className="cursor-pointer underline decoration-sky-400/70 decoration-dotted underline-offset-4 hover:bg-sky-900/40"
                title={`단어장: ${s.term.target}${s.term.note ? ` (${s.term.note})` : ""}\n클릭하면 번역문에 넣습니다.`}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => insertTerm(s.term!)}
              >
                {s.text}
              </span>
            ) : (
              s.text
            ),
          )}
        </div>
      </div>

      <div className="min-w-0 pt-[18px]">
        <textarea
          ref={textarea}
          onFocus={() => (touched.current = true)}
          value={draft}
          onChange={(e) => {
            setDraft(e.target.value);
            setReplaceHint(false);
          }}
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
        {issues.length > 0 && (
          <div className="mt-0.5 flex flex-wrap gap-1 text-[11px]">
            <span className="text-amber-400">단어장 불일치:</span>
            {issues.map((t) => (
              <button
                key={t.source}
                className="rounded bg-amber-600/20 px-1 text-amber-200 hover:bg-amber-600/40"
                title="번역문에 남은 원문 용어나, 번역문에서 선택한 부분을 지정 번역으로 바꿉니다"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => replaceTerm(t)}
              >
                {t.source} → {t.target}
              </button>
            ))}
            {replaceHint && <span className="text-zinc-400">번역문에서 바꿀 부분을 선택한 뒤 다시 클릭하세요.</span>}
          </div>
        )}
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
