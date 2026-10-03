import { create } from "zustand";
import type { FilterOptions } from "../lib/filter";

interface FilterState extends FilterOptions {
  set: (patch: Partial<FilterOptions>) => void;
  toggleFile: (file: string, additive: boolean) => void;
  reset: () => void;
}

const initial: FilterOptions = {
  query: "",
  regex: false,
  caseSensitive: false,
  scope: "both",
  status: "all",
  files: [],
  kinds: [],
  glossaryIssues: false,
  pinnedIds: null,
};

export const useFilter = create<FilterState>((set, get) => ({
  ...initial,
  set: (patch) => set(patch),
  /** additive(Ctrl+클릭)면 다중 선택, 아니면 해당 파일만 */
  toggleFile: (file, additive) => {
    const files = get().files;
    if (additive) {
      set({ files: files.includes(file) ? files.filter((f) => f !== file) : [...files, file] });
    } else {
      set({ files: files.length === 1 && files[0] === file ? [] : [file] });
    }
  },
  reset: () => set(initial),
}));
