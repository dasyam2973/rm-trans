import { useMemo } from "react";
import { statusOf } from "../../lib/status";
import { useProject } from "../../stores/projectStore";

const ENGINE_LABEL = { mv: "RPG Maker MV", mz: "RPG Maker MZ", unknown: "엔진 미확인" };

export function StatusBar() {
  const { project, entries, translations, overrides, dirty } = useProject();
  const done = useMemo(
    () => entries.filter((e) => statusOf(e, translations, overrides) === "translated").length,
    [entries, translations, overrides],
  );

  if (!project) return <div className="border-t border-zinc-800 bg-zinc-900 px-3 py-1 text-xs text-zinc-500">폴더를 열어 주세요.</div>;

  const pct = entries.length ? ((done / entries.length) * 100).toFixed(1) : "0";
  return (
    <div className="flex items-center gap-4 border-t border-zinc-800 bg-zinc-900 px-3 py-1 text-xs text-zinc-400">
      <span>{ENGINE_LABEL[project.engine]}</span>
      <span className="truncate" title={project.root}>
        {project.root}
      </span>
      <span className="ml-auto">
        번역 {done.toLocaleString()} / {entries.length.toLocaleString()} ({pct}%)
      </span>
      {dirty && <span className="text-amber-400">● 저장 안 됨</span>}
    </div>
  );
}
