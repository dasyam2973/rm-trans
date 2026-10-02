// Rust 쪽 직렬화 타입과 맞춰야 한다 (src-tauri/src/model.rs, store.rs, ai/*)

export type Kind =
  | "name"
  | "nickname"
  | "profile"
  | "description"
  | "message"
  | "term"
  | "speaker"
  | "dialogue"
  | "scroll"
  | "choice"
  | "displayName"
  | "pluginParam"
  | "pluginCommand";

/** 잘못 번역하면 게임이 깨질 수 있는 플러그인 데이터인지 */
export const isPluginKind = (k: Kind) => k === "pluginParam" || k === "pluginCommand";

export interface Entry {
  /** "{file}#{pointer}" */
  id: string;
  file: string;
  path: string;
  kind: Kind;
  group: string;
  groupLabel: string;
  context?: string;
  original: string;
}

export type Status = "translated" | "untranslated";

export interface SavedEntry {
  translation?: string;
  status?: Status;
}

export interface ProjectOptions {
  /** 플러그인 파라미터/커맨드도 추출할지 */
  includePlugins: boolean;
}

export interface ProjectFile {
  version: number;
  entries: Record<string, SavedEntry>;
  options?: ProjectOptions;
}

export type Engine = "mv" | "mz" | "unknown";

export interface Extracted {
  entries: Entry[];
  /** 추출은 계속했지만 알려야 하는 문제 */
  warnings: string[];
}

export interface OpenedProject extends Extracted {
  root: string;
  dataDir: string;
  engine: Engine;
  saved: ProjectFile | null;
}

export interface ExportReport {
  filesCopied: number;
  filesPatched: number;
  stringsApplied: number;
  skipped: string[];
}

export interface AiSettings {
  baseUrl: string;
  apiKey: string;
  model: string;
  systemPrompt: string;
  temperature: number | null;
  batchSize: number;
  concurrency: number;
  targetLanguage: string;
  responseMode: ResponseMode;
}

export type ResponseMode = "jsonObject" | "structured" | "none";

export const RESPONSE_MODE_LABELS: Record<ResponseMode, string> = {
  jsonObject: "JSON 모드 (response_format: json_object)",
  structured: "구조화된 데이터 (response_format: json_schema)",
  none: "포맷 지정 없음 (프롬프트 지시에만 의존)",
};

/** value는 프롬프트에 그대로 들어가는 영문 이름 */
export const TARGET_LANGUAGES: { value: string; label: string }[] = [
  { value: "Korean", label: "한국어" },
  { value: "English", label: "English" },
  { value: "Japanese", label: "日本語" },
  { value: "Simplified Chinese", label: "简体中文" },
  { value: "Traditional Chinese", label: "繁體中文" },
  { value: "Spanish", label: "Español" },
  { value: "French", label: "Français" },
  { value: "German", label: "Deutsch" },
  { value: "Italian", label: "Italiano" },
  { value: "Portuguese", label: "Português" },
  { value: "Russian", label: "Русский" },
  { value: "Vietnamese", label: "Tiếng Việt" },
  { value: "Thai", label: "ไทย" },
  { value: "Indonesian", label: "Bahasa Indonesia" },
];

export interface AiItem {
  id: string;
  text: string;
  group: string;
  context?: string;
}

export interface AiSummary {
  translated: number;
  failed: number;
  errors: string[];
  cancelled: boolean;
}

export const KIND_LABELS: Record<Kind, string> = {
  name: "이름",
  nickname: "닉네임",
  profile: "프로필",
  description: "설명",
  message: "메시지",
  term: "용어",
  speaker: "화자",
  dialogue: "대사",
  scroll: "스크롤",
  choice: "선택지",
  displayName: "맵 표시명",
  pluginParam: "플러그인 파라미터",
  pluginCommand: "플러그인 커맨드",
};
