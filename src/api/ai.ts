import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AiItem, AiSettings, AiSummary } from "../types";

export const getAiSettings = () => invoke<AiSettings>("get_ai_settings");
export const saveAiSettings = (settings: AiSettings) => invoke<void>("save_ai_settings", { settings });
export const defaultSystemPrompt = () => invoke<string>("default_system_prompt");

export const aiTranslate = (settings: AiSettings, items: AiItem[]) =>
  invoke<AiSummary>("ai_translate", { settings, items });
export const aiCancel = () => invoke<void>("ai_cancel");

export const onAiResult = (cb: (items: { id: string; text: string }[]) => void) =>
  listen<{ id: string; text: string }[]>("ai://result", (e) => cb(e.payload));

export const onAiProgress = (cb: (p: { done: number; total: number }) => void) =>
  listen<{ done: number; total: number }>("ai://progress", (e) => cb(e.payload));
