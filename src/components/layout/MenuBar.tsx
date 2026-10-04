import { useAssets } from "../../stores/assetStore";
import { useProject } from "../../stores/projectStore";
import { useSelection } from "../../stores/selectionStore";
import { Button } from "../ui";

export type DialogKind =
  | "saveAs"
  | "bulk"
  | "glossary"
  | "aiSettings"
  | "aiTranslate"
  | "plugins"
  | "detailed"
  | "locale"
  | "assetKeys"
  | "assetExport";

export type Tab = "translate" | "assets";

const TAB_LABELS: Record<Tab, string> = { translate: "번역", assets: "리소스" };

export function MenuBar({
  tab,
  onTab,
  onOpen,
  onSave,
  onDialog,
  onTogglePlugins,
  onToggleDetailed,
}: {
  tab: Tab;
  onTab: (t: Tab) => void;
  onOpen: () => void;
  onSave: () => void;
  onDialog: (d: DialogKind) => void;
  onTogglePlugins: () => void;
  onToggleDetailed: () => void;
}) {
  const project = useProject((s) => s.project);
  const dirty = useProject((s) => s.dirty);
  const includePlugins = useProject((s) => s.options.includePlugins);
  const detailed = useProject((s) => s.options.detailed);
  const localePairCount = useProject((s) => s.options.localePairs.length);
  const selectedCount = useSelection((s) => s.selected.size);
  const glossaryCount = useProject((s) => s.glossary.length);
  const loaded = project !== null;

  return (
    <div className="flex items-center gap-1.5 border-b border-zinc-800 bg-zinc-900 px-3 py-2">
      <span className="mr-3 font-semibold text-sky-400">RM Trans</span>
      <div className="mr-3 flex rounded border border-zinc-700 p-0.5">
        {(Object.keys(TAB_LABELS) as Tab[]).map((t) => (
          <button
            key={t}
            onClick={() => onTab(t)}
            className={`rounded px-3 py-0.5 text-sm ${tab === t ? "bg-sky-600 text-white" : "text-zinc-400 hover:text-zinc-200"}`}
          >
            {TAB_LABELS[t]}
          </button>
        ))}
      </div>
      <Button onClick={onOpen} title="Ctrl+O">
        폴더 열기
      </Button>
      <Button onClick={onSave} disabled={!loaded || !dirty} title="Ctrl+S">
        작업 저장
      </Button>
      <Button onClick={() => onDialog("saveAs")} disabled={!loaded}>
        다른 이름으로 저장
      </Button>

      {tab === "translate" ? (
        <>
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
          <Button
            onClick={onToggleDetailed}
            disabled={!loaded}
            className={detailed ? "border-amber-500! bg-amber-600/25! text-amber-200!" : ""}
            title={
              detailed
                ? "세부 수정 중입니다. 클릭하면 외부 JSON과 플러그인 설정값을 목록에서 제외합니다 (입력한 번역은 보존)."
                : "게임 폴더의 다른 JSON 파일과 플러그인의 숫자/불리언 같은 설정값도 추출합니다. 잘못 수정하면 게임이 깨질 수 있습니다."
            }
          >
            {detailed ? "⚠ 세부 수정" : "세부 수정"}
          </Button>
          <Button
            onClick={() => onDialog("locale")}
            disabled={!loaded}
            title="번역 플러그인의 언어별 JSON 파일(원본 언어 → 대상 언어)을 등록합니다."
          >
            언어 파일{localePairCount > 0 && ` (${localePairCount})`}
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
        </>
      ) : (
        <AssetTools onDialog={onDialog} />
      )}
    </div>
  );
}

/** 리소스 탭 전용 버튼 */
function AssetTools({ onDialog }: { onDialog: (d: DialogKind) => void }) {
  const scan = useAssets((s) => s.scan);
  const selectedCount = useAssets((s) => s.selected.size);
  const encrypted = scan?.files.some((f) => f.encrypted) ?? false;

  return (
    <>
      <div className="mx-2 h-5 w-px bg-zinc-700" />
      <Button onClick={() => onDialog("assetKeys")} disabled={!scan || !encrypted}>
        암호화 키
      </Button>
      <Button variant="primary" onClick={() => onDialog("assetExport")} disabled={!scan || scan.files.length === 0}>
        리소스 추출{selectedCount > 0 && ` (${selectedCount})`}
      </Button>
    </>
  );
}
