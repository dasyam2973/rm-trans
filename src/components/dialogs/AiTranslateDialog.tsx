import { useMemo, useState } from "react";
import { aiCancel, aiTranslate, getAiSettings, onAiProgress, onAiResult } from "../../api/ai";
import { statusOf } from "../../lib/status";
import { useProject } from "../../stores/projectStore";
import { useSelection } from "../../stores/selectionStore";
import type { AiSummary, Entry } from "../../types";
import { Button, Dialog } from "../ui";

type Scope = "selected" | "filteredUntranslated" | "filtered";

export function AiTranslateDialog({ filtered, onClose }: { filtered: Entry[]; onClose: () => void }) {
  const { entries, translations, overrides } = useProject();
  const selected = useSelection((s) => s.selected);
  const [scope, setScope] = useState<Scope>(selected.size > 0 ? "selected" : "filteredUntranslated");
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState({ done: 0, total: 0 });
  const [summary, setSummary] = useState<AiSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  const targets = useMemo(() => {
    switch (scope) {
      // 그룹이 이어지도록 원래 순서를 유지
      case "selected":
        return entries.filter((e) => selected.has(e.id));
      case "filteredUntranslated":
        return filtered.filter((e) => statusOf(e, translations, overrides) === "untranslated");
      case "filtered":
        return filtered;
    }
  }, [scope, entries, selected, filtered, translations, overrides]);

  const start = async () => {
    setError(null);
    setSummary(null);
    setRunning(true);
    setProgress({ done: 0, total: targets.length });
    const unlisten = await Promise.all([
      onAiResult((items) =>
        useProject.getState().setTranslations(Object.fromEntries(items.map((i) => [i.id, i.text])), {
          clearOverride: true,
        }),
      ),
      onAiProgress(setProgress),
    ]);
    try {
      const settings = await getAiSettings();
      const items = targets.map((e) => ({ id: e.id, text: e.original, group: e.group, context: e.context }));
      setSummary(await aiTranslate(settings, items));
    } catch (e) {
      setError(String(e));
    } finally {
      unlisten.forEach((u) => u());
      setRunning(false);
    }
  };

  const pct = progress.total ? (progress.done / progress.total) * 100 : 0;
  const scopes: [Scope, string, number][] = [
    ["selected", "선택한 항목", selected.size],
    ["filteredUntranslated", "현재 목록의 미번역 항목", filtered.filter((e) => statusOf(e, translations, overrides) === "untranslated").length],
    ["filtered", "현재 목록 전체 (번역된 항목 덮어쓰기)", filtered.length],
  ];

  return (
    <Dialog
      title="AI 번역"
      onClose={running ? undefined : onClose}
      footer={
        running ? (
          <Button variant="danger" onClick={() => aiCancel()}>
            중지
          </Button>
        ) : (
          <>
            <Button variant="ghost" onClick={onClose}>
              닫기
            </Button>
            <Button variant="primary" onClick={start} disabled={targets.length === 0}>
              번역 시작 ({targets.length}개)
            </Button>
          </>
        )
      }
    >
      <div className="mb-4 space-y-1.5">
        {scopes.map(([v, label, n]) => (
          <label key={v} className="flex items-center gap-2 text-sm">
            <input
              type="radio"
              className="accent-sky-500"
              checked={scope === v}
              disabled={running || n === 0}
              onChange={() => setScope(v)}
            />
            <span className={n === 0 ? "text-zinc-500" : ""}>{label}</span>
            <span className="text-xs text-zinc-500">{n.toLocaleString()}개</span>
          </label>
        ))}
      </div>
      <p className="mb-4 text-xs text-zinc-500">
        원문을 기준으로 번역하며, 결과는 도착하는 대로 번역문에 반영됩니다. 같은 그룹(대사 블록 등)은 가능한 한 같은 요청으로
        묶어 보냅니다.
      </p>

      {(running || summary) && (
        <div className="mb-3">
          <div className="mb-1 flex justify-between text-xs text-zinc-400">
            <span>{running ? "번역 중…" : "완료"}</span>
            <span>
              {progress.done} / {progress.total}
            </span>
          </div>
          <div className="h-1.5 rounded bg-zinc-800">
            <div className="h-full rounded bg-sky-500 transition-all" style={{ width: `${pct}%` }} />
          </div>
        </div>
      )}

      {summary && (
        <div className="space-y-1 text-sm">
          <p>
            성공 <span className="text-emerald-400">{summary.translated}</span> · 실패{" "}
            <span className={summary.failed ? "text-rose-400" : ""}>{summary.failed}</span>
            {summary.cancelled && <span className="ml-2 text-amber-400">(중지됨)</span>}
          </p>
          {summary.errors.length > 0 && (
            <pre className="max-h-40 overflow-auto rounded bg-zinc-950 p-2 text-xs whitespace-pre-wrap text-rose-300">
              {summary.errors.join("\n\n")}
            </pre>
          )}
        </div>
      )}
      {error && <p className="text-sm text-rose-400">{error}</p>}
    </Dialog>
  );
}
