import { invoke } from "@tauri-apps/api/core";
import type { ExportReport, Extracted, LocalePair, OpenedProject, ProjectFile, ProjectOptions } from "../types";

/** 저장된 작업 상태의 추출 옵션을 따라 추출한다 */
export const openProject = (path: string) => invoke<OpenedProject>("open_project", { path });

/** 추출 옵션을 바꿨을 때 아이템 목록만 다시 추출한다 */
export const extractEntries = (root: string, options: ProjectOptions) =>
  invoke<Extracted>("extract_entries", { root, options });

export const saveProject = (root: string, project: ProjectFile) => invoke<void>("save_project", { root, project });

/** translations: 아이템 ID → 번역문 (원문과 다른 것만), translatedOnly: 번역이 적용된 파일만 내보낸다,
 * localePairs: 대상 언어 파일은 원본 언어 파일로 만든다 */
export const exportProject = (
  root: string,
  dest: string,
  translations: Record<string, string>,
  translatedOnly: boolean,
  localePairs: LocalePair[],
) => invoke<ExportReport>("export_project", { root, dest, translations, translatedOnly, localePairs });
