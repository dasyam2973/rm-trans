import { create } from "zustand";

interface SelectionState {
  selected: Set<string>;
  /** Shift+클릭 범위 선택의 기준점 */
  anchor: string | null;
  toggle: (id: string) => void;
  /** 현재 보이는 목록(order)에서 anchor부터 id까지 선택 */
  selectRange: (order: string[], id: string) => void;
  setMany: (ids: string[], on: boolean) => void;
  clear: () => void;
}

export const useSelection = create<SelectionState>((set, get) => ({
  selected: new Set(),
  anchor: null,

  toggle: (id) => {
    const selected = new Set(get().selected);
    if (selected.has(id)) selected.delete(id);
    else selected.add(id);
    set({ selected, anchor: id });
  },

  selectRange: (order, id) => {
    const { anchor } = get();
    const a = anchor ? order.indexOf(anchor) : -1;
    const b = order.indexOf(id);
    if (a < 0 || b < 0) return get().toggle(id);
    const selected = new Set(get().selected);
    for (let i = Math.min(a, b); i <= Math.max(a, b); i++) selected.add(order[i]);
    set({ selected });
  },

  setMany: (ids, on) => {
    const selected = new Set(get().selected);
    for (const id of ids) {
      if (on) selected.add(id);
      else selected.delete(id);
    }
    set({ selected });
  },

  clear: () => set({ selected: new Set(), anchor: null }),
}));
