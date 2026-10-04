import { useMemo } from "react";
import { folderOf, useAssets } from "../../stores/assetStore";
import type { AssetFile } from "../../types";

/** 폴더별 리소스 목록. 폴더를 고르면 그 폴더의 파일만 보인다 (하위 폴더 제외) */
export function AssetSidebar({ files, webDir }: { files: AssetFile[]; webDir: string }) {
  const folder = useAssets((s) => s.folder);
  const folders = useMemo(() => {
    const map = new Map<string, { total: number; encrypted: number }>();
    for (const f of files) {
      const dir = folderOf(f.path);
      let s = map.get(dir);
      if (!s) map.set(dir, (s = { total: 0, encrypted: 0 }));
      s.total++;
      if (f.encrypted) s.encrypted++;
    }
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [files]);
  const prefix = webDir ? `${webDir}/` : "";
  const pick = (f: string | null) => useAssets.getState().set({ folder: f });

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-zinc-800 bg-zinc-900/40">
      <div className="px-3 py-2 text-xs text-zinc-400">폴더 ({folders.length})</div>
      <div className="flex-1 overflow-y-auto pb-2">
        <Item label="전체" count={files.length} active={folder === null} onClick={() => pick(null)} />
        {folders.map(([dir, s]) => (
          <Item
            key={dir}
            label={dir.startsWith(prefix) ? dir.slice(prefix.length) : dir}
            title={`${dir}\n암호화 ${s.encrypted}개 / 전체 ${s.total}개`}
            count={s.total}
            active={folder === dir}
            onClick={() => pick(folder === dir ? null : dir)}
          />
        ))}
      </div>
    </aside>
  );
}

function Item({
  label,
  title,
  count,
  active,
  onClick,
}: {
  label: string;
  title?: string;
  count: number;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      title={title}
      onClick={onClick}
      className={`flex w-full items-center justify-between gap-2 px-3 py-1 text-left text-sm ${
        active ? "bg-sky-600/25 text-sky-100" : "text-zinc-300 hover:bg-zinc-800"
      }`}
    >
      <span className="truncate">{label}</span>
      <span className="shrink-0 text-xs text-zinc-500">{count}</span>
    </button>
  );
}
