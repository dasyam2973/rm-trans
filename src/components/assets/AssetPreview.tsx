import { useEffect, useMemo, useState, type DependencyList, type SyntheticEvent } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readAsset, readAssetReplacement, removeAssetReplacement, saveAsset, setAssetReplacement } from "../../api/assets";
import { assetKeys, fileNameOf, formatSize, isReplaceable, useAssets } from "../../stores/assetStore";
import { useProject } from "../../stores/projectStore";
import { ASSET_KIND_LABELS } from "../../types";
import { Button } from "../ui";
import { SchemeBadge } from "./AssetView";

const MIME: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  webp: "image/webp",
  gif: "image/gif",
  bmp: "image/bmp",
  ogg: "audio/ogg",
  m4a: "audio/mp4",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  webm: "video/webm",
  mp4: "video/mp4",
};

/** 동영상은 통째로 메모리에 올리므로 너무 크면 미리보지 않는다 */
const MAX_PREVIEW_BYTES = 100 * 1024 * 1024;

type View = "original" | "translated";

/** load가 돌려준 바이트로 Blob URL을 만들고, 바뀌거나 사라지면 해제한다 */
function useBlobUrl(load: (() => Promise<ArrayBuffer>) | null, mime: string, deps: DependencyList) {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setUrl(null);
    setError(null);
    if (!load) return;
    let cancelled = false;
    let objectUrl: string | null = null;
    load()
      .then((buf) => {
        if (cancelled) return;
        objectUrl = URL.createObjectURL(new Blob([buf], { type: mime }));
        setUrl(objectUrl);
      })
      .catch((e) => !cancelled && setError(String(e)));
    return () => {
      cancelled = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, deps);
  return { url, error };
}

export function AssetPreview() {
  const { root, scan, preview, replaced, replacedVersion } = useAssets();
  const userKeys = useProject((s) => s.assetKeys);
  const file = useMemo(() => scan?.files.find((f) => f.path === preview) ?? null, [scan, preview]);
  const keys = useMemo(() => assetKeys(scan, userKeys), [scan, userKeys]);
  const hasReplacement = !!file && replaced.has(file.plainPath);
  const previewable = !!file && file.kind !== "font" && file.size <= MAX_PREVIEW_BYTES;
  const ext = file ? file.plainPath.slice(file.plainPath.lastIndexOf(".") + 1).toLowerCase() : "";
  const mime = MIME[ext] ?? "application/octet-stream";

  const original = useBlobUrl(
    root && file && previewable ? () => readAsset(root, file.path, keys) : null,
    mime,
    [root, file, previewable, keys],
  );
  const translated = useBlobUrl(
    root && file && hasReplacement ? () => readAssetReplacement(root, file.path) : null,
    "image/png",
    [root, file, hasReplacement, replacedVersion],
  );

  const [view, setView] = useState<View>("original");
  const [dims, setDims] = useState<Partial<Record<View, string>>>({});
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  useEffect(() => {
    setView(hasReplacement ? "translated" : "original");
    setDims({});
    setActionError(null);
    // 파일을 바꿀 때만 초기화한다
  }, [file?.path]);

  if (!file || !root) {
    return (
      <aside className="flex w-96 shrink-0 items-center justify-center border-l border-zinc-800 text-sm text-zinc-500">
        파일을 선택하면 미리 볼 수 있습니다.
      </aside>
    );
  }

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setActionError(null);
    try {
      await action();
    } catch (e) {
      setActionError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const saveOriginal = () =>
    run(async () => {
      const dest = await save({ defaultPath: fileNameOf(file.plainPath), filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
      if (dest) await saveAsset(root, file.path, keys, dest);
    });

  const pickReplacement = () =>
    run(async () => {
      const source = await open({ title: "번역 이미지 선택", filters: [{ name: "PNG", extensions: ["png"] }] });
      if (typeof source !== "string") return;
      const plain = await setAssetReplacement(root, file.path, source);
      useAssets.getState().setReplaced(plain, true);
      setDims((d) => ({ original: d.original }));
      setView("translated");
    });

  const removeReplacement = () =>
    run(async () => {
      if (!confirm("번역 이미지를 제거할까요? .rmtrans/assets의 파일이 삭제됩니다.")) return;
      await removeAssetReplacement(root, file.path);
      useAssets.getState().setReplaced(file.plainPath, false);
      setView("original");
    });

  const shown = view === "translated" && hasReplacement ? translated : original;
  const sizeMismatch = hasReplacement && dims.original && dims.translated && dims.original !== dims.translated;
  const onDims = (v: View) => (e: SyntheticEvent<HTMLImageElement>) => {
    const size = `${e.currentTarget.naturalWidth} × ${e.currentTarget.naturalHeight}`;
    setDims((d) => ({ ...d, [v]: size }));
  };

  return (
    <aside className="flex w-96 shrink-0 flex-col border-l border-zinc-800">
      {hasReplacement && (
        <div className="flex gap-1 border-b border-zinc-800 p-2">
          {(["original", "translated"] as View[]).map((v) => (
            <button
              key={v}
              onClick={() => setView(v)}
              className={`flex-1 rounded px-2 py-0.5 text-sm ${
                view === v ? "bg-sky-600/30 text-sky-100" : "text-zinc-400 hover:bg-zinc-800"
              }`}
            >
              {v === "original" ? "원본" : "번역 이미지"}
              {dims[v] && <span className="ml-1 text-xs text-zinc-500">{dims[v]}</span>}
            </button>
          ))}
        </div>
      )}
      <div className="flex min-h-0 flex-1 items-center justify-center overflow-auto p-3">
        {shown.error ? (
          <p className="text-sm whitespace-pre-wrap text-rose-400">{shown.error}</p>
        ) : !previewable ? (
          <p className="text-sm text-zinc-500">
            {file.kind === "font" ? "폰트는 미리볼 수 없습니다." : "파일이 커서 미리볼 수 없습니다."}
          </p>
        ) : !shown.url ? (
          <p className="text-sm text-zinc-500">불러오는 중…</p>
        ) : file.kind === "image" ? null : file.kind === "audio" ? (
          <audio src={shown.url} controls autoPlay className="w-full" />
        ) : (
          <video src={shown.url} controls className="max-h-full max-w-full" />
        )}
        {/* 크기 비교를 위해 원본과 번역 이미지를 둘 다 불러 두고 하나만 보여준다 */}
        {file.kind === "image" &&
          (["original", "translated"] as View[]).map((v) => {
            const src = (v === "original" ? original : translated).url;
            return (
              src && (
                <img
                  key={v}
                  src={src}
                  alt=""
                  onLoad={onDims(v)}
                  className={`checkerboard max-h-full max-w-full object-contain [image-rendering:pixelated] ${
                    shown.url === src && !shown.error ? "" : "hidden"
                  }`}
                />
              )
            );
          })}
      </div>
      {sizeMismatch && (
        <p className="border-t border-amber-900 bg-amber-950/40 px-3 py-1.5 text-xs text-amber-200">
          ⚠ 번역 이미지 크기({dims.translated})가 원본({dims.original})과 다릅니다. 게임에서 위치가 어긋날 수 있습니다.
        </p>
      )}
      <dl className="grid grid-cols-[64px_1fr] gap-x-2 gap-y-1 border-t border-zinc-800 p-3 text-xs">
        <dt className="text-zinc-500">이름</dt>
        <dd className="break-all text-zinc-200">{fileNameOf(file.plainPath)}</dd>
        <dt className="text-zinc-500">경로</dt>
        <dd className="break-all text-zinc-400">{file.path}</dd>
        <dt className="text-zinc-500">종류</dt>
        <dd className="text-zinc-400">
          {ASSET_KIND_LABELS[file.kind]}
          {!hasReplacement && dims.original && ` · ${dims.original}`} · {formatSize(file.size)}
        </dd>
        <dt className="text-zinc-500">암호화</dt>
        <dd>
          <SchemeBadge file={file} />
        </dd>
      </dl>
      <div className="flex flex-wrap gap-1.5 border-t border-zinc-800 p-3">
        <Button onClick={saveOriginal} disabled={busy} title="복호화한 원본을 파일로 저장합니다 (편집용).">
          원본 저장…
        </Button>
        {isReplaceable(file) && (
          <>
            <Button
              variant="primary"
              onClick={pickReplacement}
              disabled={busy}
              title="편집한 PNG를 이 이미지의 번역본으로 등록합니다. '다른 이름으로 저장' 시 원본과 같은 방식으로 암호화되어 적용됩니다."
            >
              {hasReplacement ? "번역 이미지 교체…" : "번역 이미지 지정…"}
            </Button>
            {hasReplacement && (
              <Button variant="ghost" onClick={removeReplacement} disabled={busy}>
                제거
              </Button>
            )}
          </>
        )}
        {actionError && <p className="w-full text-xs whitespace-pre-wrap text-rose-400">{actionError}</p>}
      </div>
    </aside>
  );
}
