import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { basename, dirname, join } from "@tauri-apps/api/path";
import { exportAssets, onAssetProgress } from "../../api/assets";
import { assetKeys, filterAssets, useAssets } from "../../stores/assetStore";
import type { AssetExportReport } from "../../types";
import { Button, Dialog, Field, inputClass } from "../ui";

type Target = "selected" | "filtered" | "all";

/** 리소스를 복호화해 원래 폴더 구조 그대로 새 폴더에 저장한다. */
export function AssetExportDialog({ onClose }: { onClose: () => void }) {
  const { root, scan, replaced, selected, folder, query, kinds, crypt, replacedOnly } = useAssets();
  const files = useMemo(() => scan?.files ?? [], [scan]);
  const filtered = useMemo(
    () => filterAssets(files, replaced, { folder, query, kinds, crypt, replacedOnly }),
    [files, replaced, folder, query, kinds, crypt, replacedOnly],
  );
  const [target, setTarget] = useState<Target>(selected.size > 0 ? "selected" : "filtered");
  const [includePlain, setIncludePlain] = useState(true);
  const [parent, setParent] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<AssetExportReport | null>(null);

  const targets = useMemo(() => {
    const pick = (list: typeof files) => list.filter((f) => includePlain || f.encrypted).map((f) => f.path);
    return {
      selected: pick(files.filter((f) => selected.has(f.path))),
      filtered: pick(filtered),
      all: pick(files),
    };
  }, [files, filtered, selected, includePlain]);
  const paths = targets[target];

  useEffect(() => {
    if (!root) return;
    (async () => {
      setParent(await dirname(root));
      setName(`${await basename(root)}_resources`);
    })();
  }, [root]);

  useEffect(() => {
    const unlisten = onAssetProgress(setProgress);
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const pickParent = async () => {
    const dir = await open({ directory: true, defaultPath: parent });
    if (typeof dir === "string") setParent(dir);
  };

  const run = async () => {
    if (!root) return;
    setBusy(true);
    setError(null);
    setProgress(null);
    try {
      const dest = await join(parent, name.trim());
      setReport(await exportAssets(root, dest, paths, assetKeys(scan)));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const options: { value: Target; label: string }[] = [
    { value: "selected", label: "선택한 파일" },
    { value: "filtered", label: "목록에 보이는 파일" },
    { value: "all", label: "전체" },
  ];

  return (
    <Dialog
      title="리소스 추출"
      onClose={busy ? undefined : onClose}
      footer={
        report ? (
          <Button onClick={onClose}>닫기</Button>
        ) : (
          <>
            <Button variant="ghost" onClick={onClose} disabled={busy}>
              취소
            </Button>
            <Button variant="primary" onClick={run} disabled={busy || paths.length === 0 || !parent || !name.trim()}>
              {busy ? "추출 중…" : `추출 (${paths.length.toLocaleString()}개)`}
            </Button>
          </>
        )
      }
    >
      {report ? (
        <div className="space-y-1 text-sm">
          <p className="text-emerald-400">추출했습니다.</p>
          <p>
            저장한 파일: {report.written.toLocaleString()}개 (복호화 {report.decrypted.toLocaleString()}개)
          </p>
          {report.failed.length > 0 && (
            <details className="text-amber-400">
              <summary>실패한 파일 {report.failed.length}개</summary>
              <pre className="mt-1 max-h-40 overflow-auto text-xs whitespace-pre-wrap text-zinc-400">
                {report.failed.join("\n")}
              </pre>
            </details>
          )}
        </div>
      ) : (
        <>
          <p className="mb-3 text-sm text-zinc-400">
            암호화된 리소스를 복호화해 원래 확장자(.png, .ogg 등)로, 원래 폴더 구조 그대로 새 폴더에 저장합니다. 원본 폴더는
            바뀌지 않습니다.
          </p>
          <div className="mb-3 flex flex-col gap-1.5 text-sm">
            {options.map((o) => (
              <label key={o.value} className="flex items-center gap-2">
                <input
                  type="radio"
                  className="accent-sky-500"
                  checked={target === o.value}
                  disabled={busy}
                  onChange={() => setTarget(o.value)}
                />
                {o.label}
                <span className="text-zinc-500">({targets[o.value].length.toLocaleString()}개)</span>
              </label>
            ))}
          </div>
          <label className="mb-3 flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              className="accent-sky-500"
              checked={includePlain}
              disabled={busy}
              onChange={(e) => setIncludePlain(e.target.checked)}
            />
            암호화되지 않은 리소스도 포함
          </label>
          <Field label="저장할 위치">
            <div className="flex gap-2">
              <input className={`${inputClass} flex-1`} value={parent} onChange={(e) => setParent(e.target.value)} />
              <Button onClick={pickParent} disabled={busy}>
                찾아보기
              </Button>
            </div>
          </Field>
          <Field label="새 폴더 이름" hint="이미 있는 폴더라면 비어 있어야 합니다.">
            <input className={inputClass} value={name} onChange={(e) => setName(e.target.value)} />
          </Field>
          {busy && progress && (
            <div className="mb-2">
              <div className="h-1.5 rounded bg-zinc-800">
                <div className="h-full rounded bg-sky-500" style={{ width: `${(progress.done / progress.total) * 100}%` }} />
              </div>
              <p className="mt-1 text-xs text-zinc-500">
                {progress.done.toLocaleString()} / {progress.total.toLocaleString()}
              </p>
            </div>
          )}
          {error && <p className="text-sm text-rose-400">{error}</p>}
        </>
      )}
    </Dialog>
  );
}
