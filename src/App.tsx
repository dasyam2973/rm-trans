import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { extractEntries, openProject, saveProject } from "./api/project";
import { MenuBar, type DialogKind } from "./components/layout/MenuBar";
import { StatusBar } from "./components/layout/StatusBar";
import { FilterBar } from "./components/filter/FilterBar";
import { FileSidebar } from "./components/filter/FileSidebar";
import { EntryList } from "./components/entries/EntryList";
import { SaveAsDialog } from "./components/dialogs/SaveAsDialog";
import { BulkEditDialog } from "./components/dialogs/BulkEditDialog";
import { AiSettingsDialog } from "./components/dialogs/AiSettingsDialog";
import { AiTranslateDialog } from "./components/dialogs/AiTranslateDialog";
import { PluginWarningDialog } from "./components/dialogs/PluginWarningDialog";
import { DetailedWarningDialog } from "./components/dialogs/DetailedWarningDialog";
import { LocalePairsDialog } from "./components/dialogs/LocalePairsDialog";
import { GlossaryDialog } from "./components/dialogs/GlossaryDialog";
import { filterEntries } from "./lib/filter";
import { useGlossaryMatcher } from "./lib/glossary";
import { useFilter } from "./stores/filterStore";
import { useProject } from "./stores/projectStore";
import { useSelection } from "./stores/selectionStore";
import { isPluginKind, type ProjectOptions } from "./types";

export default function App() {
  const { project, entries, translations, overrides } = useProject();
  const filter = useFilter();
  const glossary = useGlossaryMatcher();
  const [dialog, setDialog] = useState<DialogKind | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const { result: filtered, error: filterError } = useMemo(
    () => filterEntries(entries, translations, overrides, filter, glossary),
    [entries, translations, overrides, filter, glossary],
  );

  const handleOpen = useCallback(async () => {
    if (useProject.getState().dirty && !confirm("저장하지 않은 변경 사항이 있습니다. 무시하고 다른 폴더를 열까요?")) return;
    const dir = await open({ directory: true, title: "RPG Maker 게임 폴더 선택" });
    if (typeof dir !== "string") return;
    setLoading(true);
    setError(null);
    setNotice(null);
    try {
      const opened = await openProject(dir);
      useProject.getState().load(opened);
      useSelection.getState().clear();
      useFilter.getState().reset();
      if (opened.warnings.length) setNotice(opened.warnings.join("\n"));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const handleSave = useCallback(async () => {
    const s = useProject.getState();
    if (!s.project) return;
    // 편집 중인 입력란의 내용을 먼저 반영 (blur 시점에 커밋됨)
    (document.activeElement as HTMLElement | null)?.blur();
    try {
      await saveProject(s.project.root, useProject.getState().toProjectFile());
      useProject.getState().markSaved();
    } catch (e) {
      setError(String(e));
    }
  }, []);

  /** 추출 옵션(플러그인 포함, 세부 수정)을 바꾸고 아이템 목록을 다시 추출한다. */
  const changeOptions = useCallback(async (changes: Partial<ProjectOptions>) => {
    const s = useProject.getState();
    if (!s.project) return;
    (document.activeElement as HTMLElement | null)?.blur();
    setError(null);
    setNotice(null);
    const options = { ...s.options, ...changes };
    try {
      const { entries, warnings } = await extractEntries(s.project.root, options);
      useProject.getState().reloadEntries(entries, options);
      useSelection.getState().clear();
      // 더 이상 없는 파일/종류로 걸러져서 목록이 비지 않도록 정리
      const f = useFilter.getState();
      const files = new Set(entries.map((e) => e.file));
      const kinds = new Set(entries.map((e) => e.kind));
      f.set({ files: f.files.filter((x) => files.has(x)), kinds: f.kinds.filter((k) => kinds.has(k)) });

      const messages = [...warnings];
      if (changes.includePlugins && !entries.some((e) => isPluginKind(e.kind))) messages.push("추출된 플러그인 데이터가 없습니다.");
      if (changes.detailed && !entries.some((e) => e.kind === "jsonData")) messages.push("추출된 외부 JSON 데이터가 없습니다.");
      if (messages.length) setNotice(messages.join("\n"));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  // 끌 때는 번역이 보존되므로 바로 적용하고, 켤 때만 위험성을 알린다
  const handleTogglePlugins = useCallback(() => {
    if (useProject.getState().options.includePlugins) changeOptions({ includePlugins: false });
    else setDialog("plugins");
  }, [changeOptions]);

  const handleToggleDetailed = useCallback(() => {
    if (useProject.getState().options.detailed) changeOptions({ detailed: false });
    else setDialog("detailed");
  }, [changeOptions]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const key = e.key.toLowerCase();
      if (key === "s") {
        e.preventDefault();
        handleSave();
      } else if (key === "o") {
        e.preventDefault();
        handleOpen();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [handleOpen, handleSave]);

  const close = () => setDialog(null);

  return (
    <div className="flex h-full flex-col">
      <MenuBar
        onOpen={handleOpen}
        onSave={handleSave}
        onDialog={setDialog}
        onTogglePlugins={handleTogglePlugins}
        onToggleDetailed={handleToggleDetailed}
      />

      {error && (
        <div className="flex items-start gap-2 border-b border-rose-900 bg-rose-950/60 px-3 py-1.5 text-sm text-rose-200">
          <span className="flex-1 whitespace-pre-wrap">{error}</span>
          <button onClick={() => setError(null)}>✕</button>
        </div>
      )}
      {notice && (
        <div className="flex items-start gap-2 border-b border-amber-900 bg-amber-950/50 px-3 py-1.5 text-sm text-amber-200">
          <span className="flex-1 whitespace-pre-wrap">{notice}</span>
          <button onClick={() => setNotice(null)}>✕</button>
        </div>
      )}

      {project ? (
        <>
          <FilterBar filtered={filtered} total={entries.length} error={filterError} />
          <div className="flex min-h-0 flex-1">
            <FileSidebar />
            <EntryList filtered={filtered} />
          </div>
        </>
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 text-zinc-500">
          {loading ? (
            <p>불러오는 중…</p>
          ) : (
            <>
              <p>RPG Maker MV/MZ 게임 폴더를 열어 주세요.</p>
              <p className="text-xs">게임 루트, www 폴더, data 폴더 중 아무거나 선택해도 됩니다.</p>
            </>
          )}
        </div>
      )}

      <StatusBar />

      {dialog === "saveAs" && project && <SaveAsDialog onClose={close} />}
      {dialog === "bulk" && <BulkEditDialog onClose={close} />}
      {dialog === "glossary" && project && <GlossaryDialog onClose={close} />}
      {dialog === "aiSettings" && <AiSettingsDialog onClose={close} />}
      {dialog === "aiTranslate" && project && <AiTranslateDialog filtered={filtered} onClose={close} />}
      {dialog === "plugins" && project && (
        <PluginWarningDialog
          onClose={close}
          onConfirm={() => {
            close();
            changeOptions({ includePlugins: true });
          }}
        />
      )}
      {dialog === "locale" && project && (
        <LocalePairsDialog
          onClose={close}
          onApply={(localePairs) => {
            close();
            changeOptions({ localePairs });
          }}
        />
      )}
      {dialog === "detailed" && project && (
        <DetailedWarningDialog
          onClose={close}
          onConfirm={() => {
            close();
            changeOptions({ detailed: true });
          }}
        />
      )}
    </div>
  );
}
