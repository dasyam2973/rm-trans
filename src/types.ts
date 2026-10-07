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
  | "locale"
  | "pictureText"
  | "stringArg"
  | "dbValue"
  | "scriptText"
  | "scriptArg";

/** 플러그인 데이터인지 */
export const isPluginKind = (k: Kind) => k === "pluginParam" || k === "pluginCommand";

/** 잘못 번역하면 게임이 깨질 수 있는 데이터인지 (플러그인 데이터, 세부 수정의 외부 JSON, WOLF RPG 문자열 인수) */
export const isRiskyKind = (k: Kind) => isPluginKind(k) || k === "jsonData" || k === "stringArg";

/** WOLF RPG의 DB 문자열·문자열 인수 중 공백 없는 영문/숫자 토큰 (smile1, BZ_waitA, initialize …).
 * 내부 식별자일 가능성이 높지만 Lv·SP 같은 표시 용어도 섞여 있어 추출은 하고 위험 항목으로 다룬다 */
export const isIdentLike = (e: Entry) =>
  (e.kind === "dbValue" || e.kind === "stringArg") && /^[A-Za-z0-9_-]+$/.test(e.original.trim());

/** AI 번역에서 기본으로 빼고 경고를 표시하는 항목 */
export const isRiskyEntry = (e: Entry) => isRiskyKind(e.kind) || isIdentLike(e);

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
  /** 게임 자체 스크립트 등 텍스트 파일 추출 규칙 (엔진과 무관) */
  textRules: TextRule[];
}

/** 텍스트 파일 추출 규칙 (src-tauri/src/textfile.rs).
 * 접두로 시작하지 않는 줄이 이어진 묶음 하나, 지정한 명령의 인자 토큰 하나가 각각 항목이 된다 */
export interface TextRule {
  /** 게임 루트 기준 파일 패턴 ('/' 구분, * ** ?) */
  pattern: string;
  /** 이 문자열로 시작하는 줄은 명령·주석이라 번역하지 않는다 */
  skipPrefixes: string[];
  /** 인자에 표시 문장이 들어 있는 명령 */
  argCommands: ArgCommand[];
}

/** command로 시작하는 줄을 공백으로 나눴을 때 arg번째(명령 자신이 0) 토큰이 표시 문장이다 */
export interface ArgCommand {
  command: string;
  arg: number;
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

/** wolf2: WOLF RPG 2.x (Shift-JIS), wolf3: WOLF RPG 3.x (UTF-8) */
export type Engine = "mv" | "mz" | "unknown" | "wolf2" | "wolf3";

export const ENGINE_LABELS: Record<Engine, string> = {
  mv: "RPG Maker MV",
  mz: "RPG Maker MZ",
  unknown: "엔진 미확인",
  wolf2: "WOLF RPG 2.x",
  wolf3: "WOLF RPG 3.x",
};

/** WOLF RPG에는 RPG Maker 전용 옵션(플러그인, 세부 수정, 언어 파일)이 없다 */
export const isWolf = (e: Engine) => e === "wolf2" || e === "wolf3";

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
  /** WOLF RPG: 풀린 폴더가 있어서 복사하지 않은 .wolf 아카이브 수 */
  archivesSkipped: number;
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

/** AI 번역 시 제어 문자 마스킹 규칙 (엔진별) */
export type Dialect = "rpgm" | "wolf";

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
  pictureText: "문자열 그림",
  stringArg: "문자열 인수",
  dbValue: "DB 문자열",
  scriptText: "스크립트 문장",
  scriptArg: "스크립트 인자",
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
