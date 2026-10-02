import { useProject } from "../../stores/projectStore";
import { useSelection } from "../../stores/selectionStore";
import { Button } from "../ui";

export type DialogKind = "saveAs" | "bulk" | "glossary" | "aiSettings" | "aiTranslate" | "plugins";

export function MenuBar({
  onOpen,
  onSave,
  onDialog,
  onTogglePlugins,
}: {
  onOpen: () => void;
  onSave: () => void;
  onDialog: (d: DialogKind) => void;
  onTogglePlugins: () => void;
}) {
  const project = useProject((s) => s.project);
  const dirty = useProject((s) => s.dirty);
  const includePlugins = useProject((s) => s.options.includePlugins);
  const selectedCount = useSelection((s) => s.selected.size);
  const glossaryCount = useProject((s) => s.glossary.length);
  const loaded = project !== null;

  return (
    <div className="flex items-center gap-1.5 border-b border-zinc-800 bg-zinc-900 px-3 py-2">
      <span className="mr-3 font-semibold text-sky-400">RM Trans</span>
      <Button onClick={onOpen} title="Ctrl+O">
        폴더 열기
      </Button>
      <Button onClick={onSave} disabled={!loaded || !dirty} title="Ctrl+S">
        작업 저장
      </Button>
      <Button onClick={() => onDialog("saveAs")} disabled={!loaded}>
        다른 이름으로 저장
      </Button>

      <div className="mx-2 h-5 w-px bg-zinc-700" />

      <Button
        onClick={onTogglePlugins}
        disabled={!loaded}
        className={includePlugins ? "border-amber-500! bg-amber-600/25! text-amber-200!" : ""}
        title={
          includePlugins
            ? "플러그인 데이터를 추출하는 중입니다. 클릭하면 목록에서 제외합니다 (입력한 번역은 보존)."
            : "플러그인 파라미터/커맨드도 추출합니다. 잘못 수정하면 게임이 깨질 수 있습니다."
        }
      >
        {includePlugins ? "⚠ 플러그인 포함" : "플러그인 포함"}
      </Button>

      <div className="mx-2 h-5 w-px bg-zinc-700" />

      <Button onClick={() => onDialog("bulk")} disabled={selectedCount === 0}>
        일괄 수정{selectedCount > 0 && ` (${selectedCount})`}
      </Button>
      <Button onClick={() => onDialog("glossary")} disabled={!loaded}>
        단어장{glossaryCount > 0 && ` (${glossaryCount})`}
      </Button>
      <Button variant="primary" onClick={() => onDialog("aiTranslate")} disabled={!loaded}>
        AI 번역
      </Button>
      <Button variant="ghost" onClick={() => onDialog("aiSettings")}>
        AI 설정
      </Button>
    </div>
  );
}
