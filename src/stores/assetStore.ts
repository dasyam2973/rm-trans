import { create } from "zustand";
import { scanAssets } from "../api/assets";
import type { AssetFile, AssetKind, AssetScan, SchemeKey } from "../types";
import { useProject } from "./projectStore";

/** 암호화 상태 필터 */
export type CryptFilter = "all" | "encrypted" | "plain";

interface AssetState {
  /** 스캔한 게임 루트. 다른 폴더를 열면 다시 스캔한다 */
  root: string | null;
  scan: AssetScan | null;
  loading: boolean;
  error: string | null;
  /** 번역 이미지가 등록된 리소스의 plainPath */
  replaced: Set<string>;
  /** 번역 이미지를 바꿀 때마다 올려서 미리보기를 다시 읽게 한다 */
  replacedVersion: number;

  folder: string | null;
  query: string;
  kinds: AssetKind[];
  crypt: CryptFilter;
  /** 번역 이미지가 있는 것만 */
  replacedOnly: boolean;
  selected: Set<string>;
  /** Shift+클릭 범위 선택의 기준점 */
  anchor: string | null;
  /** 미리보기 중인 파일 경로 */
  preview: string | null;

  load: (root: string) => Promise<void>;
  reset: () => void;
  setReplaced: (plainPath: string, on: boolean) => void;
  set: (patch: Partial<Pick<AssetState, "folder" | "query" | "kinds" | "crypt" | "replacedOnly" | "preview">>) => void;
  toggle: (path: string) => void;
  selectRange: (order: string[], path: string) => void;
  setMany: (paths: string[], on: boolean) => void;
}

const initialView = () => ({
  folder: null,
  query: "",
  kinds: [] as AssetKind[],
  crypt: "all" as CryptFilter,
  replacedOnly: false,
  selected: new Set<string>(),
  anchor: null,
  preview: null,
});

export const useAssets = create<AssetState>((set, get) => ({
  root: null,
  scan: null,
  loading: false,
  error: null,
  replaced: new Set(),
  replacedVersion: 0,
  ...initialView(),

  load: async (root) => {
    set({ root, scan: null, loading: true, error: null, replaced: new Set(), ...initialView() });
    try {
      const scan = await scanAssets(root);
      if (get().root === root) set({ scan, replaced: new Set(scan.replacements) });
    } catch (e) {
      if (get().root === root) set({ error: String(e) });
    } finally {
      if (get().root === root) set({ loading: false });
    }
  },

  reset: () => set({ root: null, scan: null, loading: false, error: null, replaced: new Set(), ...initialView() }),

  setReplaced: (plainPath, on) => {
    const replaced = new Set(get().replaced);
    if (on) replaced.add(plainPath);
    else replaced.delete(plainPath);
    set({ replaced, replacedVersion: get().replacedVersion + 1 });
  },

  set: (patch) => set(patch),

  toggle: (path) => {
    const selected = new Set(get().selected);
    if (selected.has(path)) selected.delete(path);
    else selected.add(path);
    set({ selected, anchor: path });
  },

  selectRange: (order, path) => {
    const { anchor } = get();
    const a = anchor ? order.indexOf(anchor) : -1;
    const b = order.indexOf(path);
    if (a < 0 || b < 0) return get().toggle(path);
    const selected = new Set(get().selected);
    for (let i = Math.min(a, b); i <= Math.max(a, b); i++) selected.add(order[i]);
    set({ selected });
  },

  setMany: (paths, on) => {
    const selected = new Set(get().selected);
    for (const p of paths) {
      if (on) selected.add(p);
      else selected.delete(p);
    }
    set({ selected });
  },
}));

/** 방식별로 실제 쓸 키 (사용자 입력 > 감지). 사용자 입력 키는 project.json에 저장된다 */
export function assetKeys(scan: AssetScan | null, userKeys = useProject.getState().assetKeys): SchemeKey[] {
  const keys: SchemeKey[] = [];
  for (const s of scan?.schemes ?? []) {
    const key = userKeys[s.scheme] ?? s.key;
    if (key) keys.push({ scheme: s.scheme, key });
  }
  return keys;
}

/** 파일이 들어 있는 폴더 (루트 기준) */
export const folderOf = (path: string) => path.slice(0, Math.max(0, path.lastIndexOf("/")));

export const fileNameOf = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** 번역 이미지를 등록할 수 있는 리소스인지 (PNG 이미지) */
export const isReplaceable = (file: AssetFile) => file.plainPath.toLowerCase().endsWith(".png");

export function filterAssets(
  files: AssetFile[],
  replaced: Set<string>,
  f: { folder: string | null; query: string; kinds: AssetKind[]; crypt: CryptFilter; replacedOnly: boolean },
): AssetFile[] {
  const q = f.query.trim().toLowerCase();
  return files.filter(
    (file) =>
      (f.folder === null || folderOf(file.path) === f.folder) &&
      (f.kinds.length === 0 || f.kinds.includes(file.kind)) &&
      (f.crypt === "all" || (f.crypt === "encrypted") === file.encrypted) &&
      (!f.replacedOnly || replaced.has(file.plainPath)) &&
      (!q || file.path.toLowerCase().includes(q)),
  );
}

export function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
