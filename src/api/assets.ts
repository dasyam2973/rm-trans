import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AssetExportReport, AssetScan, Scheme, SchemeKey } from "../types";

export const scanAssets = (root: string) => invoke<AssetScan>("scan_assets", { root });

/** System.json 형식의 키 문자열 → 방식별 실제 키(hex) */
export const deriveAssetKey = (scheme: Scheme, source: string) => invoke<string>("derive_asset_key", { scheme, source });

/** 복호화한 파일 내용 (평문 파일은 그대로) */
export const readAsset = (root: string, path: string, keys: SchemeKey[]) =>
  invoke<ArrayBuffer>("read_asset", { root, path, keys });

/** paths의 리소스를 복호화해 dest에 원래 확장자로 쓴다 */
export const exportAssets = (root: string, dest: string, paths: string[], keys: SchemeKey[]) =>
  invoke<AssetExportReport>("export_assets", { root, dest, paths, keys });

export const onAssetProgress = (cb: (p: { done: number; total: number }) => void) =>
  listen<{ done: number; total: number }>("assets://progress", (e) => cb(e.payload));

/** 리소스 하나를 복호화해 dest 파일로 저장 (번역 이미지 편집용) */
export const saveAsset = (root: string, path: string, keys: SchemeKey[], dest: string) =>
  invoke<void>("save_asset", { root, path, keys, dest });

/** source PNG를 path 리소스의 번역 이미지로 등록 (.rmtrans/assets에 복사). 등록된 plainPath를 돌려준다 */
export const setAssetReplacement = (root: string, path: string, source: string) =>
  invoke<string>("set_asset_replacement", { root, path, source });

export const removeAssetReplacement = (root: string, path: string) =>
  invoke<void>("remove_asset_replacement", { root, path });

export const readAssetReplacement = (root: string, path: string) =>
  invoke<ArrayBuffer>("read_asset_replacement", { root, path });
