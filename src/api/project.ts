import { invoke } from "@tauri-apps/api/core";
import type { ExportReport, OpenedProject, ProjectFile } from "../types";

export const openProject = (path: string) => invoke<OpenedProject>("open_project", { path });

export const saveProject = (root: string, project: ProjectFile) => invoke<void>("save_project", { root, project });

/** translations: 아이템 ID → 번역문 (원문과 다른 것만) */
export const exportProject = (root: string, dest: string, translations: Record<string, string>) =>
  invoke<ExportReport>("export_project", { root, dest, translations });
