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
  | "pluginCommand"
  | "jsonData"
  | "locale";

/** 플러그인 데이터인지 */
export const isPluginKind = (k: Kind) => k === "pluginParam" || k === "pluginCommand";

/** 잘못 번역하면 게임이 깨질 수 있는 데이터인지 (플러그인 데이터, 세부 수정의 외부 JSON) */
export const isRiskyKind = (k: Kind) => isPluginKind(k) || k === "jsonData";

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
  /** 언어 파일 쌍의 대상 파일에 이미 있던 값. 저장된 번역이 없으면 번역으로 채운다 */
  initial?: string;
}

export type Status = "translated" | "untranslated";

export interface SavedEntry {
  translation?: string;
  status?: Status;
}

export interface ProjectOptions {
  /** 플러그인 파라미터/커맨드도 추출할지 */
  includePlugins: boolean;
  /** 세부 수정: 게임 폴더의 다른 JSON 파일도 추출하고, 플러그인 데이터의 숫자/불리언 같은 값도 걸러내지 않는다 */
  detailed: boolean;
  /** 번역 플러그인의 언어 파일 쌍 */
  localePairs: LocalePair[];
}

/** 원본 언어 파일 → 대상 언어 파일 (게임 루트 기준 상대 경로, '/' 구분).
 * 원본 파일 구조로 항목을 만들고, 내보낼 때 원본에 번역을 적용해 대상 파일로 쓴다 (누락된 항목은 원본 언어로 채워짐) */
export interface LocalePair {
  source: string;
  target: string;
}

/** 단어장 항목. 인명/고유명사 등을 일관되게 번역하도록 AI 요청에 함께 보낸다 */
export interface GlossaryTerm {
  source: string;
  target: string;
  /** 성별, 말투 등 참고 메모 (AI에도 전달) */
  note?: string;
}

export interface ProjectFile {
  version: number;
  entries: Record<string, SavedEntry>;
  options?: ProjectOptions;
  glossary?: GlossaryTerm[];
  /** 사용자가 직접 입력한 리소스 암호화 키 (감지한 키보다 우선) */
  assetKeys?: SchemeKey[];
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
  /** 번역 이미지를 쓴 파일 수 */
  imagesApplied: number;
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
  /** 원문의 제어 문자가 빠지거나 중복된 번역을 실패로 처리할지 */
  requireCodes: boolean;
  /** 대사 블록이 통째로 번역 대상이면 줄을 이어 번역한 뒤 다시 나눌지 */
  mergeLines: boolean;
  /** 병합 번역 결과를 나눌 때 한 줄의 최대 폭 (전각 글자 수 기준, 반각은 0.5) */
  maxLineWidth: number;
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
  /** 대사 줄이고 그 블록의 대사 줄이 모두 이번 요청에 들어 있는지 (병합 번역 대상) */
  merge?: boolean;
}

export interface AiSummary {
  translated: number;
  failed: number;
  /** 제어 문자 외에 번역할 텍스트가 없어 보내지 않은 항목 수 */
  skipped: number;
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
  jsonData: "JSON 데이터",
  locale: "언어 파일",
};

// ── 리소스 (src-tauri/src/rpgm/assets.rs, crypto.rs, commands/assets.rs) ──

export type Scheme = "standard" | "arthran";

export const SCHEME_LABELS: Record<Scheme, string> = {
  standard: "RPG Maker 내장",
  arthran: "Arthran Decrypterator",
};

/** 방식별 실제 XOR 키 길이 (바이트) */
export const SCHEME_KEY_BYTES: Record<Scheme, number> = { standard: 16, arthran: 32 };

export type AssetKind = "image" | "audio" | "video" | "font";

export const ASSET_KIND_LABELS: Record<AssetKind, string> = {
  image: "이미지",
  audio: "오디오",
  video: "동영상",
  font: "폰트",
};

export interface AssetFile {
  /** 루트 기준 상대 경로, '/' 구분 */
  path: string;
  /** 복호화했을 때의 경로 (원래 확장자). 평문이면 path와 같다 */
  plainPath: string;
  kind: AssetKind;
  /** 암호화 확장자인지 */
  encrypted: boolean;
  /** 헤더로 판별한 방식. 암호화 확장자인데 null이면 모르는 방식 */
  scheme: Scheme | null;
  size: number;
}

export type KeySource = "system" | "recovered" | "none";

export interface SchemeInfo {
  scheme: Scheme;
  fileCount: number;
  /** 실제 XOR 키 (hex) */
  key: string | null;
  keySource: KeySource;
  /** System.json 키와 PNG에서 복구한 키가 다름 (복구한 키를 씀) */
  keyMismatch: boolean;
}

export interface AssetScan {
  /** 리소스 폴더의 루트 기준 상대 경로 ("www" 또는 "") */
  webDir: string;
  /** System.json의 encryptionKey 원문 */
  systemKey: string | null;
  schemes: SchemeInfo[];
  /** 켜져 있는 암호화 관련 플러그인 */
  cryptoPlugins: string[];
  files: AssetFile[];
  /** 번역 이미지가 등록된 리소스의 plainPath */
  replacements: string[];
  warnings: string[];
}

export interface SchemeKey {
  scheme: Scheme;
  key: string;
}

export interface AssetExportReport {
  written: number;
  decrypted: number;
  failed: string[];
}
