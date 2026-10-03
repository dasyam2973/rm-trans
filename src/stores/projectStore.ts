import { create } from "zustand";
import type { Engine, Entry, GlossaryTerm, OpenedProject, ProjectFile, ProjectOptions, SavedEntry, Status } from "../types";

interface ProjectInfo {
  root: string;
  dataDir: string;
  engine: Engine;
}

interface ProjectState {
  project: ProjectInfo | null;
  options: ProjectOptions;
  entries: Entry[];
  entryById: Map<string, Entry>;
  /** 원문과 다른 번역문만 저장한다 */
  translations: Record<string, string>;
  /** 사용자가 직접 지정한 상태 */
  overrides: Record<string, Status>;
  /** 저장 파일에는 있지만 현재 게임 데이터에서 찾을 수 없는 항목. 잃어버리지 않도록 저장 시 그대로 유지 */
  orphans: Record<string, SavedEntry>;
  /** 인명/고유명사 단어장 */
  glossary: GlossaryTerm[];
  dirty: boolean;

  load: (p: OpenedProject) => void;
  /** 추출 옵션을 바꿔 다시 추출한 목록으로 교체한다. 기존 번역은 유지된다 */
  reloadEntries: (entries: Entry[], options: ProjectOptions) => void;
  setTranslations: (updates: Record<string, string>, opts?: { clearOverride?: boolean }) => void;
  setOverrides: (ids: string[], status: Status | null) => void;
  setGlossary: (glossary: GlossaryTerm[]) => void;
  markSaved: () => void;
  toProjectFile: () => ProjectFile;
  /** 내보내기용: 원문과 다른 번역문 전체 */
  exportMap: () => Record<string, string>;
}

const DEFAULT_OPTIONS: ProjectOptions = { includePlugins: false, detailed: false, localePairs: [] };

/** 저장된 항목을 현재 아이템 목록 기준으로 번역문/상태/고아 항목으로 나눈다.
 * 저장된 항목이 없으면 언어 파일에 이미 있던 값(initial)을 번역으로 쓴다. */
function distribute(entries: Entry[], saved: Record<string, SavedEntry>) {
  const entryById = new Map(entries.map((e) => [e.id, e]));
  const translations: Record<string, string> = {};
  const overrides: Record<string, Status> = {};
  const orphans: Record<string, SavedEntry> = {};
  for (const e of entries) if (e.initial !== undefined && !(e.id in saved)) translations[e.id] = e.initial;
  for (const [id, s] of Object.entries(saved)) {
    const entry = entryById.get(id);
    if (!entry) {
      orphans[id] = s;
      continue;
    }
    if (s.translation !== undefined && s.translation !== entry.original) translations[id] = s.translation;
    if (s.status) overrides[id] = s.status;
  }
  return { entries, entryById, translations, overrides, orphans };
}

export const useProject = create<ProjectState>((set, get) => ({
  project: null,
  options: DEFAULT_OPTIONS,
  entries: [],
  entryById: new Map(),
  translations: {},
  overrides: {},
  orphans: {},
  glossary: [],
  dirty: false,

  load: (p) => {
    set({
      project: { root: p.root, dataDir: p.dataDir, engine: p.engine },
      options: { ...DEFAULT_OPTIONS, ...p.saved?.options },
      ...distribute(p.entries, p.saved?.entries ?? {}),
      glossary: p.saved?.glossary ?? [],
      dirty: false,
    });
  },

  reloadEntries: (entries, options) => {
    // 빠지는 아이템의 번역은 고아 항목으로 옮겨 두었다가, 다시 추출되면 되살린다
    set({ options, ...distribute(entries, get().toProjectFile().entries), dirty: true });
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

  setGlossary: (glossary) => set({ glossary, dirty: true }),

  markSaved: () => set({ dirty: false }),

  toProjectFile: () => {
    const { entries: list, translations, overrides, orphans, options, glossary } = get();
    const entries: Record<string, SavedEntry> = { ...orphans };
    // 언어 파일의 기존 값을 원문으로 되돌린 경우, 다시 열 때 기존 값이 채워지지 않도록 원문을 번역으로 기록해 둔다
    for (const e of list) if (e.initial !== undefined && !(e.id in translations)) entries[e.id] = { translation: e.original };
    for (const [id, translation] of Object.entries(translations)) entries[id] = { translation };
    for (const [id, status] of Object.entries(overrides)) entries[id] = { ...entries[id], status };
    return { version: 1, entries, options, glossary };
  },

  exportMap: () => ({ ...get().translations }),
}));
