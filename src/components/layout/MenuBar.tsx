import { useEffect, useRef, useState } from "react";
import { useAssets } from "../../stores/assetStore";
import { useProject } from "../../stores/projectStore";
import { useSelection } from "../../stores/selectionStore";
import { isWolf } from "../../types";


export type DialogKind =
  | "saveAs"
  | "bulk"
  | "glossary"
  | "aiSettings"
  | "aiTranslate"
  | "plugins"
  | "detailed"
  | "locale"
  | "textRules"
  | "assetKeys"
  | "assetExport";

export type Tab = "translate" | "assets";

const TAB_LABELS: Record<Tab, string> = { translate: "번역", assets: "리소스" };

type MenuItem =
  | "separator"
  | {
      label: string;
      onClick: () => void;
      disabled?: boolean;
      /** 체크 표시 (켜고 끄는 옵션) */
      checked?: boolean;
      shortcut?: string;
      title?: string;
      warn?: boolean;
    };

type Menu = { label: string; items: MenuItem[] };

/** 맨 위의 메뉴 바 (파일/옵션/편집 …) */
export function MenuBar({
  tab,
  onOpen,
  onSave,
  onDialog,
  onTogglePlugins,
  onToggleDetailed,
}: {
  tab: Tab;
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
  const textRuleCount = useProject((s) => s.options.textRules.length);
  const selectedCount = useSelection((s) => s.selected.size);
  const glossaryCount = useProject((s) => s.glossary.length);
  const scan = useAssets((s) => s.scan);
  const assetSelectedCount = useAssets((s) => s.selected.size);
  const encrypted = scan?.files.some((f) => f.encrypted) ?? false;
  const loaded = project !== null;
  // WOLF RPG에는 RPG Maker 전용 옵션이 없다
  const wolf = project !== null && isWolf(project.engine);
  const translating = loaded && tab === "translate";
  const inAssets = loaded && tab === "assets";

  const menus: Menu[] = [
    {
      label: "파일",
      items: [
        { label: "폴더 열기…", onClick: onOpen, shortcut: "Ctrl+O" },
        "separator",
        { label: "작업 저장", onClick: onSave, disabled: !loaded || !dirty, shortcut: "Ctrl+S" },
        { label: "다른 이름으로 저장…", onClick: () => onDialog("saveAs"), disabled: !loaded },
      ],
    },
    {
      label: "옵션",
      items: [
        {
          label: "플러그인 포함",
          onClick: onTogglePlugins,
          disabled: !loaded || wolf,
          checked: includePlugins,
          warn: true,
          title: wolf
            ? "RPG Maker 전용 옵션입니다."
            : includePlugins
            ? "플러그인 데이터를 추출하는 중입니다. 클릭하면 목록에서 제외합니다 (입력한 번역은 보존)."
            : "플러그인 파라미터/커맨드도 추출합니다. 잘못 수정하면 게임이 깨질 수 있습니다.",
        },
        {
          label: "세부 수정",
          onClick: onToggleDetailed,
          disabled: !loaded || wolf,
          checked: detailed,
          warn: true,
          title: wolf
            ? "RPG Maker 전용 옵션입니다."
            : detailed
            ? "세부 수정 중입니다. 클릭하면 외부 JSON과 플러그인 설정값을 목록에서 제외합니다 (입력한 번역은 보존)."
            : "게임 폴더의 다른 JSON 파일과 플러그인의 숫자/불리언 같은 설정값도 추출합니다. 잘못 수정하면 게임이 깨질 수 있습니다.",
        },
        "separator",
        {
          label: `언어 파일…${localePairCount > 0 ? ` (${localePairCount})` : ""}`,
          onClick: () => onDialog("locale"),
          disabled: !loaded || wolf,
          title: wolf
            ? "RPG Maker 전용 옵션입니다."
            : "번역 플러그인의 언어별 JSON 파일(원본 언어 → 대상 언어)을 등록합니다.",
        },
        {
          label: `텍스트 파일…${textRuleCount > 0 ? ` (${textRuleCount})` : ""}`,
          onClick: () => onDialog("textRules"),
          disabled: !loaded,
          title: "게임 자체 스크립트 등 텍스트 파일에서 대사를 추출하는 규칙을 등록합니다.",
        },
      ],
    },
    {
      label: "편집",
      items: [
        {
          label: `일괄 수정…${selectedCount > 0 ? ` (${selectedCount})` : ""}`,
          onClick: () => onDialog("bulk"),
          disabled: !translating || selectedCount === 0,
          title: "선택한 아이템을 한꺼번에 수정합니다.",
        },
        {
          label: `단어장…${glossaryCount > 0 ? ` (${glossaryCount})` : ""}`,
          onClick: () => onDialog("glossary"),
          disabled: !loaded,
        },
      ],
    },
    {
      label: "AI",
      items: [
        { label: "AI 번역…", onClick: () => onDialog("aiTranslate"), disabled: !translating },
        "separator",
        { label: "AI 설정…", onClick: () => onDialog("aiSettings") },
      ],
    },
    {
      label: "리소스",
      items: [
        {
          label: "암호화 키…",
          onClick: () => onDialog("assetKeys"),
          disabled: !inAssets || !scan || !encrypted,
        },
        {
          label: `리소스 추출…${assetSelectedCount > 0 ? ` (${assetSelectedCount})` : ""}`,
          onClick: () => onDialog("assetExport"),
          disabled: !inAssets || !scan || scan.files.length === 0,
        },
      ],
    },
  ];

  return (
    <div className="flex items-center border-b border-zinc-800 bg-zinc-900 px-2 text-[13px]">
      <MenuGroup menus={menus} />
    </div>
  );
}

/** 드롭다운 메뉴 묶음. 하나가 열려 있을 때는 다른 메뉴에 마우스를 올리면 그 메뉴로 바뀐다. */
function MenuGroup({ menus }: { menus: Menu[] }) {
  const [open, setOpen] = useState<number | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open === null) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(null);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(null);
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={ref} className="flex">
      {menus.map((m, i) => (
        <div key={m.label} className="relative">
          <button
            onClick={() => setOpen(open === i ? null : i)}
            onMouseEnter={() => open !== null && setOpen(i)}
            className={`rounded px-2.5 py-1 my-1 ${open === i ? "bg-zinc-700 text-white" : "text-zinc-300 hover:bg-zinc-800"}`}
          >
            {m.label}
          </button>
          {open === i && (
            <div className="absolute left-0 top-full z-40 min-w-52 rounded-md border border-zinc-700 bg-zinc-900 py-1 shadow-xl">
              {m.items.map((item, j) =>
                item === "separator" ? (
                  <div key={j} className="my-1 h-px bg-zinc-800" />
                ) : (
                  <button
                    key={j}
                    disabled={item.disabled}
                    title={item.title}
                    onClick={() => {
                      setOpen(null);
                      item.onClick();
                    }}
                    className="flex w-full items-center gap-2 px-3 py-1 text-left whitespace-nowrap text-zinc-200 hover:bg-sky-600 hover:text-white disabled:cursor-not-allowed disabled:text-zinc-600 disabled:hover:bg-transparent"
                  >
                    <span className={`w-4 ${item.warn && item.checked ? "text-amber-400" : ""}`}>
                      {item.checked ? "✓" : ""}
                    </span>
                    <span className="flex-1">{item.label}</span>
                    {item.shortcut && <span className="ml-6 text-xs text-zinc-500">{item.shortcut}</span>}
                  </button>
                ),
              )}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

/** 메뉴 바 아래의 번역/리소스 탭 줄. 위험한 추출 옵션이 켜져 있으면 오른쪽에 표시한다. */
export function TabBar({ tab, onTab }: { tab: Tab; onTab: (t: Tab) => void }) {
  const loaded = useProject((s) => s.project !== null);
  const includePlugins = useProject((s) => s.options.includePlugins);
  const detailed = useProject((s) => s.options.detailed);

  return (
    <div className="flex items-stretch gap-1 border-b border-zinc-800 bg-zinc-950 px-2">
      {(Object.keys(TAB_LABELS) as Tab[]).map((t) => (
        <button
          key={t}
          onClick={() => onTab(t)}
          className={`-mb-px border-b-2 px-4 py-1.5 text-sm ${
            tab === t ? "border-sky-500 text-zinc-100" : "border-transparent text-zinc-500 hover:text-zinc-300"
          }`}
        >
          {TAB_LABELS[t]}
        </button>
      ))}
      {loaded && (includePlugins || detailed) && (
        <div className="ml-auto flex gap-1.5 self-center text-xs">
          {includePlugins && <WarnBadge title="플러그인 데이터를 추출하는 중입니다 (옵션 메뉴에서 끌 수 있음).">⚠ 플러그인 포함</WarnBadge>}
          {detailed && <WarnBadge title="세부 수정 중입니다 (옵션 메뉴에서 끌 수 있음).">⚠ 세부 수정</WarnBadge>}
        </div>
      )}
    </div>
  );
}

function WarnBadge({ title, children }: { title: string; children: string }) {
  return (
    <span title={title} className="rounded border border-amber-600/60 bg-amber-600/15 px-1.5 py-0.5 text-amber-200">
      {children}
    </span>
  );
}
