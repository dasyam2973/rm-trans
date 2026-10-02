import { useProject } from "../../stores/projectStore";
import { useSelection } from "../../stores/selectionStore";
import { Button } from "../ui";

export type DialogKind = "saveAs" | "bulk" | "aiSettings" | "aiTranslate";

export function MenuBar({
  onOpen,
  onSave,
  onDialog,
}: {
  onOpen: () => void;
  onSave: () => void;
  onDialog: (d: DialogKind) => void;
}) {
  const project = useProject((s) => s.project);
  const dirty = useProject((s) => s.dirty);
  const selectedCount = useSelection((s) => s.selected.size);
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

      <Button onClick={() => onDialog("bulk")} disabled={selectedCount === 0}>
        일괄 수정{selectedCount > 0 && ` (${selectedCount})`}
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
