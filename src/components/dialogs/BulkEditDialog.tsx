import { useMemo, useState } from "react";
import { translationOf } from "../../lib/status";
import { useProject } from "../../stores/projectStore";
import { useSelection } from "../../stores/selectionStore";
import type { Entry, Status } from "../../types";
import { Button, Dialog, Field, Toggle, inputClass } from "../ui";

/** 선택한 아이템들의 번역문/상태를 한 번에 바꾼다. */
export function BulkEditDialog({ onClose }: { onClose: () => void }) {
  const selected = useSelection((s) => s.selected);
  const { entryById, translations, setTranslations, setOverrides } = useProject();
  const targets = useMemo(
    () => [...selected].map((id) => entryById.get(id)).filter((e): e is Entry => !!e),
    [selected, entryById],
  );
  const ids = targets.map((e) => e.id);

  const [find, setFind] = useState("");
  const [replace, setReplace] = useState("");
  const [regex, setRegex] = useState(false);
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const pattern = useMemo(() => {
    if (!find) return null;
    try {
      const source = regex ? find : find.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      return new RegExp(source, caseSensitive ? "gu" : "giu");
    } catch {
      return "error" as const;
    }
  }, [find, regex, caseSensitive]);

  const replacePreview = useMemo(() => {
    if (!pattern || pattern === "error") return 0;
    return targets.filter((e) => {
      pattern.lastIndex = 0;
      return pattern.test(translationOf(e, translations));
    }).length;
  }, [pattern, targets, translations]);

  const applyReplace = () => {
    if (!pattern || pattern === "error") return;
    const updates: Record<string, string> = {};
    for (const e of targets) {
      const cur = translationOf(e, translations);
      // 정규식이 아닐 때는 $ 등 치환 패턴이 해석되지 않도록 함수로 넘긴다
      const next = regex ? cur.replace(pattern, replace) : cur.replace(pattern, () => replace);
      if (next !== cur) updates[e.id] = next;
    }
    setTranslations(updates);
    setMessage(`${Object.keys(updates).length}개 항목을 바꿨습니다.`);
  };

  const setStatus = (status: Status | null) => {
    setOverrides(ids, status);
    setMessage(`${ids.length}개 항목의 상태를 바꿨습니다.`);
  };

  const resetToOriginal = () => {
    setTranslations(Object.fromEntries(targets.map((e) => [e.id, e.original])));
    setMessage(`${ids.length}개 항목의 번역문을 원문으로 되돌렸습니다.`);
  };

  return (
    <Dialog title={`일괄 수정 (${targets.length}개 선택)`} onClose={onClose} footer={<Button onClick={onClose}>닫기</Button>}>
      <section className="mb-5">
        <h3 className="mb-2 text-sm font-medium">상태 지정</h3>
        <div className="flex gap-2">
          <Button onClick={() => setStatus("translated")}>번역으로 지정</Button>
          <Button onClick={() => setStatus("untranslated")}>미번역으로 지정</Button>
          <Button variant="ghost" onClick={() => setStatus(null)}>
            자동 판정으로
          </Button>
        </div>
      </section>

      <section className="mb-5">
        <h3 className="mb-2 text-sm font-medium">번역문 찾아 바꾸기</h3>
        <Field label="찾을 내용">
          <div className="flex items-center gap-1">
            <input
              className={`${inputClass} flex-1 ${pattern === "error" ? "border-rose-500" : ""}`}
              value={find}
              onChange={(e) => setFind(e.target.value)}
            />
            <Toggle on={regex} onChange={setRegex} title="정규식">
              .*
            </Toggle>
            <Toggle on={caseSensitive} onChange={setCaseSensitive} title="대소문자 구분">
              Aa
            </Toggle>
          </div>
        </Field>
        <Field label="바꿀 내용" hint={regex ? "$1, $2 등으로 캡처 그룹을 참조할 수 있습니다." : undefined}>
          <input className={inputClass} value={replace} onChange={(e) => setReplace(e.target.value)} />
        </Field>
        <div className="flex items-center gap-3">
          <Button onClick={applyReplace} disabled={!pattern || pattern === "error" || replacePreview === 0}>
            바꾸기
          </Button>
          <span className="text-xs text-zinc-400">
            {pattern === "error" ? "정규식 오류" : find ? `${replacePreview}개 항목에서 일치` : ""}
          </span>
        </div>
      </section>

      <section>
        <h3 className="mb-2 text-sm font-medium">초기화</h3>
        <Button variant="danger" onClick={resetToOriginal}>
          번역문을 원문으로 되돌리기
        </Button>
      </section>

      {message && <p className="mt-4 text-sm text-emerald-400">{message}</p>}
    </Dialog>
  );
}
