import { create } from "zustand";
import type { Engine, Entry, OpenedProject, ProjectFile, SavedEntry, Status } from "../types";

interface ProjectInfo {
  root: string;
  dataDir: string;
  engine: Engine;
}

interface ProjectState {
  project: ProjectInfo | null;
  entries: Entry[];
  entryById: Map<string, Entry>;
  /** 원문과 다른 번역문만 저장한다 */
  translations: Record<string, string>;
  /** 사용자가 직접 지정한 상태 */
  overrides: Record<string, Status>;
  /** 저장 파일에는 있지만 현재 게임 데이터에서 찾을 수 없는 항목. 잃어버리지 않도록 저장 시 그대로 유지 */
  orphans: Record<string, SavedEntry>;
  dirty: boolean;

  load: (p: OpenedProject) => void;
  setTranslations: (updates: Record<string, string>, opts?: { clearOverride?: boolean }) => void;
  setOverrides: (ids: string[], status: Status | null) => void;
  markSaved: () => void;
  toProjectFile: () => ProjectFile;
  /** 내보내기용: 원문과 다른 번역문 전체 */
  exportMap: () => Record<string, string>;
}

export const useProject = create<ProjectState>((set, get) => ({
  project: null,
  entries: [],
  entryById: new Map(),
  translations: {},
  overrides: {},
  orphans: {},
  dirty: false,

  load: (p) => {
    const entryById = new Map(p.entries.map((e) => [e.id, e]));
    const translations: Record<string, string> = {};
    const overrides: Record<string, Status> = {};
    const orphans: Record<string, SavedEntry> = {};
    for (const [id, saved] of Object.entries(p.saved?.entries ?? {})) {
      const entry = entryById.get(id);
      if (!entry) {
        orphans[id] = saved;
        continue;
      }
      if (saved.translation !== undefined && saved.translation !== entry.original) translations[id] = saved.translation;
      if (saved.status) overrides[id] = saved.status;
    }
    set({
      project: { root: p.root, dataDir: p.dataDir, engine: p.engine },
      entries: p.entries,
      entryById,
      translations,
      overrides,
      orphans,
      dirty: false,
    });
  },

  setTranslations: (updates, opts) => {
    const { entryById } = get();
    const translations = { ...get().translations };
    const overrides = opts?.clearOverride ? { ...get().overrides } : get().overrides;
    for (const [id, text] of Object.entries(updates)) {
      const entry = entryById.get(id);
      if (!entry) continue;
      if (text === entry.original) delete translations[id];
      else translations[id] = text;
      if (opts?.clearOverride) delete overrides[id];
    }
    set({ translations, overrides, dirty: true });
  },

  setOverrides: (ids, status) => {
    const overrides = { ...get().overrides };
    for (const id of ids) {
      if (status) overrides[id] = status;
      else delete overrides[id];
    }
    set({ overrides, dirty: true });
  },

  markSaved: () => set({ dirty: false }),

  toProjectFile: () => {
    const { translations, overrides, orphans } = get();
    const entries: Record<string, SavedEntry> = { ...orphans };
    for (const [id, translation] of Object.entries(translations)) entries[id] = { translation };
    for (const [id, status] of Object.entries(overrides)) entries[id] = { ...entries[id], status };
    return { version: 1, entries };
  },

  exportMap: () => ({ ...get().translations }),
}));
