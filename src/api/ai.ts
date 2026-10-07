import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AiItem, AiSettings, AiSummary, Dialect, GlossaryTerm } from "../types";

export const getAiSettings = () => invoke<AiSettings>("get_ai_settings");
export const saveAiSettings = (settings: AiSettings) => invoke<void>("save_ai_settings", { settings });
export const defaultSystemPrompt = () => invoke<string>("default_system_prompt");

/** glossary: 단어장 전체 (배치마다 등장하는 용어만 골라 보내는 것은 백엔드에서 한다),
 * dialect: 제어 문자 규칙 (src-tauri/src/ai/codes.rs) */
export const aiTranslate = (settings: AiSettings, items: AiItem[], glossary: GlossaryTerm[], dialect: Dialect) =>
  invoke<AiSummary>("ai_translate", { settings, items, glossary, dialect });
export const aiCancel = () => invoke<void>("ai_cancel");

export const onAiResult = (cb: (items: { id: string; text: string }[]) => void) =>
  listen<{ id: string; text: string }[]>("ai://result", (e) => cb(e.payload));

export const onAiProgress = (cb: (p: { done: number; total: number }) => void) =>
  listen<{ done: number; total: number }>("ai://progress", (e) => cb(e.payload));
