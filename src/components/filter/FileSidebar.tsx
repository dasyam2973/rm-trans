import { useMemo, type MouseEvent } from "react";
import { statusOf } from "../../lib/status";
import { useFilter } from "../../stores/filterStore";
import { useProject } from "../../stores/projectStore";

interface FileStat {
  file: string;
  total: number;
  done: number;
}

export function FileSidebar() {
  const { project, entries, translations, overrides } = useProject();
  const { files, toggleFile, set } = useFilter();

  const stats = useMemo(() => {
    const map = new Map<string, FileStat>();
    for (const e of entries) {
      let s = map.get(e.file);
      if (!s) map.set(e.file, (s = { file: e.file, total: 0, done: 0 }));
      s.total++;
      if (statusOf(e, translations, overrides) === "translated") s.done++;
    }
    return [...map.values()];
  }, [entries, translations, overrides]);

  if (!project) return null;
  const prefix = project.dataDir ? `${project.dataDir}/` : "";

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-zinc-800 bg-zinc-900/40">
      <div className="flex items-center justify-between px-3 py-2 text-xs text-zinc-400">
        <span>파일 ({stats.length})</span>
        <span className="text-zinc-600">Ctrl+클릭: 다중 선택</span>
      </div>
      <div className="flex-1 overflow-y-auto pb-2">
        <FileItem label="전체" active={files.length === 0} onClick={() => set({ files: [] })} />
        {stats.map((s) => (
          <FileItem
            key={s.file}
            label={s.file.startsWith(prefix) ? s.file.slice(prefix.length) : s.file}
            title={s.file}
            stat={s}
            active={files.includes(s.file)}
            onClick={(e) => toggleFile(s.file, e.ctrlKey || e.metaKey)}
          />
        ))}
      </div>
    </aside>
  );
}

function FileItem({
  label,
  title,
  stat,
  active,
  onClick,
}: {
  label: string;
  title?: string;
  stat?: FileStat;
  active: boolean;
  onClick: (e: MouseEvent) => void;
}) {
  const pct = stat && stat.total ? (stat.done / stat.total) * 100 : 0;
  return (
    <button
      title={title}
      onClick={onClick}
      className={`block w-full px-3 py-1 text-left text-sm ${active ? "bg-sky-600/25 text-sky-100" : "text-zinc-300 hover:bg-zinc-800"}`}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="truncate">{label}</span>
        {stat && (
          <span className={`shrink-0 text-xs ${stat.done === stat.total ? "text-emerald-400" : "text-zinc-500"}`}>
            {stat.done}/{stat.total}
          </span>
        )}
      </div>
      {stat && (
        <div className="mt-1 h-0.5 rounded bg-zinc-800">
          <div className="h-full rounded bg-emerald-500/70" style={{ width: `${pct}%` }} />
        </div>
      )}
    </button>
  );
}
