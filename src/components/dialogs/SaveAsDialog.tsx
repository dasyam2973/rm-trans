import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { basename, dirname, join } from "@tauri-apps/api/path";
import { exportProject } from "../../api/project";
import { useProject } from "../../stores/projectStore";
import { isRiskyKind, type ExportReport } from "../../types";
import { Button, Dialog, Field, inputClass } from "../ui";

/** 원본 게임 폴더를 통째로 복사한 뒤 번역문을 적용해 새 폴더로 저장한다. (옵션: 번역된 파일만) */
export function SaveAsDialog({ onClose }: { onClose: () => void }) {
  const project = useProject((s) => s.project)!;
  const [parent, setParent] = useState("");
  const [name, setName] = useState("");
  const [translatedOnly, setTranslatedOnly] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<ExportReport | null>(null);
  const pluginCount = useMemo(() => {
    const { translations, entryById } = useProject.getState();
    return Object.keys(translations).filter((id) => {
      const kind = entryById.get(id)?.kind;
      return kind !== undefined && isRiskyKind(kind);
    }).length;
  }, []);

  useEffect(() => {
    (async () => {
      setParent(await dirname(project.root));
      setName(`${await basename(project.root)}_translated`);
    })();
  }, [project.root]);

  const pickParent = async () => {
    const dir = await open({ directory: true, defaultPath: parent });
    if (typeof dir === "string") setParent(dir);
  };

  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      const dest = await join(parent, name.trim());
      const s = useProject.getState();
      setReport(await exportProject(project.root, dest, s.exportMap(), translatedOnly, s.options.localePairs));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title="다른 이름으로 저장"
      onClose={busy ? undefined : onClose}
      footer={
        report ? (
          <Button onClick={onClose}>닫기</Button>
        ) : (
          <>
            <Button variant="ghost" onClick={onClose} disabled={busy}>
              취소
            </Button>
            <Button variant="primary" onClick={run} disabled={busy || !parent || !name.trim()}>
              {busy ? (translatedOnly ? "저장 중…" : "복사 중…") : "저장"}
            </Button>
          </>
        )
      }
    >
      {report ? (
        <div className="space-y-1 text-sm">
          <p className="text-emerald-400">저장했습니다.</p>
          {!translatedOnly && <p>복사한 파일: {report.filesCopied.toLocaleString()}개</p>}
          <p>
            번역 적용: {report.filesPatched}개 파일, {report.stringsApplied.toLocaleString()}개 문자열
          </p>
          {report.skipped.length > 0 && (
            <details className="text-amber-400">
              <summary>적용하지 못한 항목 {report.skipped.length}개</summary>
              <pre className="mt-1 max-h-40 overflow-auto text-xs text-zinc-400">{report.skipped.join("\n")}</pre>
            </details>
          )}
        </div>
      ) : (
        <>
          <p className="mb-3 text-sm text-zinc-400">
            {translatedOnly
              ? "번역이 적용된 파일만 원래 폴더 구조 그대로 새 폴더에 저장합니다. 게임 폴더에 덮어씌워 사용하세요. 원본 폴더는 바뀌지 않습니다."
              : "게임 폴더 전체를 새 폴더로 복사하고 번역문을 적용합니다. 원본 폴더는 바뀌지 않습니다."}
          </p>
          <label className="mb-3 flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              className="accent-sky-500"
              checked={translatedOnly}
              disabled={busy}
              onChange={(e) => setTranslatedOnly(e.target.checked)}
            />
            번역된 파일만 내보내기
          </label>
          {pluginCount > 0 && (
            <p className="mb-3 rounded border border-amber-600/60 bg-amber-950/40 px-3 py-2 text-sm text-amber-200">
              ⚠ 플러그인·JSON 데이터 번역 {pluginCount.toLocaleString()}개가 적용됩니다. 저장한 뒤 게임을 실행해 정상적으로
              동작하는지 꼭 확인하세요.
            </p>
          )}
          <Field label="저장할 위치">
            <div className="flex gap-2">
              <input className={`${inputClass} flex-1`} value={parent} onChange={(e) => setParent(e.target.value)} />
              <Button onClick={pickParent}>찾아보기</Button>
            </div>
          </Field>
          <Field label="새 폴더 이름" hint="이미 있는 폴더라면 비어 있어야 합니다.">
            <input className={inputClass} value={name} onChange={(e) => setName(e.target.value)} />
          </Field>
          {error && <p className="text-sm text-rose-400">{error}</p>}
        </>
      )}
    </Dialog>
  );
}
