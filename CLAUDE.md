# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

RM Trans: a Tauri 2 desktop app for translating RPG Maker MV/MZ games. It extracts translatable strings from a game's JSON data (and optionally `js/plugins.js`), lets the user edit/AI-translate them, and exports a translated copy of the game. Frontend is React 19 + TypeScript + Vite + Tailwind v4 + Zustand; backend is Rust (`src-tauri/`). UI strings and code comments are in Korean — keep new comments in Korean to match.

## Commands

Prerequisites: Node.js, Rust, and the Tauri 2 system prerequisites; run `npm install` first.

- `npm run tauri dev` — run the app (starts Vite on port 1420 + Rust backend)
- `npm run tauri build` — release bundle
- `npm run build` — frontend typecheck (`tsc`) + Vite build; this is the only TS check (no linter/formatter configured)
- `cd src-tauri && cargo test` — Rust tests (all tests are inline `#[cfg(test)]` modules; there are no frontend tests)
- Single test: `cd src-tauri && cargo test extract_then_export_roundtrip` (or any test name substring)

## Architecture

### Data flow
1. **Open** (`commands/project.rs::open_project`): `rpgm/detect.rs` locates the data folder (`www/data` for deployed MV, `data` for editor projects, or the data folder itself) and engine; `rpgm/extract/` walks System → DB files (`DB_ORDER`) → `MapXXX.json` → optionally plugins, producing `Entry` items; `store.rs` loads saved work state. Event command lists (map events, common events, troops) all go through `extract/events.rs::extract_list`, which decides which command codes are translatable. MV plugin commands (code 356) are intentionally not extracted, because they are one raw command string and translating part of it breaks the command.
2. **Edit** happens entirely in the frontend Zustand stores (`src/stores/`).
3. **Save** writes `<game root>/.rmtrans/project.json` (atomic tmp+rename, `BTreeMap` for stable diffs).
4. **Export** (`commands/export.rs`) writes to an empty destination outside the source folder. By default it copies the whole game tree (minus `.rmtrans`) and patches strings in place; with `translated_only` it skips the copy, reads each patched file from the source, and writes only files with applied translations at the same relative path. The original game folder is never modified.

### Entry identity
Every translatable string maps 1:1 to a JSON string value. Its ID is `"{file}#{json-pointer}"` where `file` is relative to the game root with `/` separators. This ID is the key in `project.json`, the export translation map, and AI requests — changing how pointers are built for an extractor orphans existing saved translations. Saved entries whose ID no longer exists are kept as "orphans" in `projectStore` and re-saved so they aren't lost (and are revived if re-extracted, e.g. toggling plugin extraction).

### Byte-preserving patching (key invariant)
`jsonspan.rs` is a custom minimal JSON parser that records each value's byte span. `rpgm/apply.rs` replaces only the spans of translated strings, so untouched bytes (key order, whitespace, number formatting like `1.50`) remain identical to the original. Do not switch export to `serde_json` reserialization. Strings are encoded like JS `JSON.stringify` (non-ASCII not escaped).

- **Nested JSON strings**: plugin parameters often contain JSON encoded inside a string. A pointer segment `~j` (`jsonspan::NESTED`) means "parse this string value as JSON and continue the pointer inside it". `apply::patch` handles this recursively and re-encodes the outer string.
- **plugins.js**: `rpgm/plugins_js.rs::json_range` finds the `$plugins = [...]` array so it can be treated as JSON; `apply::patch_file` dispatches on `.js` extension.

### Detailed mode (세부 수정)
`ProjectOptions.detailed` is saved in `project.json`, like `includePlugins`. When it is on:
- `extract/json_files.rs` walks the whole game root. It extracts every non-empty string, as `Kind::JsonData`, from `.json` files that aren't standard RPG Maker data. It skips data-folder DB/Map/MapInfos/Animations/Tilesets files (`is_standard_data_file`), `package.json` (the NW.js manifest), hidden folders, and `node_modules`. It reuses `plugins::walk`, so nested JSON strings (`~j`) are followed.
- Plugin params and commands skip the `is_text` filter (`Target.all`), so numeric and boolean-looking strings are extracted too. When a nested JSON string contains no strings (e.g. `"[1,2]"`), the outer string itself becomes the entry.
- Only JSON string values are extracted. Real JSON numbers and booleans are never extracted, because `apply.rs` only patches strings.

### Locale pairs (번역 플러그인 언어 파일)
`ProjectOptions.localePairs` maps a source-language JSON file to a target-language JSON file. These are for translation plugins that keep one JSON file per language.
- **Extraction** (`extract/locale.rs`): entries follow the *source* file's structure, but their IDs use the *target* file path. The original text is the source value. A non-empty value already at the same pointer in the target becomes `Entry.initial`. The frontend uses it as the translation only when `project.json` has no saved entry for that ID. If the user reverts such an entry to the original, `toProjectFile` stores `{translation: original}`, so the old value doesn't come back on the next open. Target-only keys produce a warning. Paired files are excluded from detailed-mode `jsonData`.
- **Export**: `export_project` receives the pairs. Each target file is built by patching the *source* file's bytes and writing the result to the target path. The target is always written, even with no translations, so missing keys get filled with source-language text.

### Grouping
`Entry.group` (built via `Sink::group_id` from an anchor node path) ties related items together — e.g. a speaker line (code 101) and its following dialogue lines (401), or a choice list (102). The UI displays groups together and `ai/runner.rs::make_batches` tries to keep a group within one AI batch.

### Status
There is no stored "translated" flag by default: an entry is translated iff its translation differs from the original (`src/lib/status.ts`). Only translations that differ from the original are stored/exported; an explicit user `status` override is stored separately.

### AI translation
`ai/` targets any OpenAI-compatible chat API (configurable base URL/model). Settings persist to the app config dir (`ai-settings.json`, with legacy migration in `settings.rs`). `runner.rs` batches items, runs requests concurrently (`buffer_unordered`), and streams results to the frontend via Tauri events `ai://result` and `ai://progress`; cancellation goes through the managed `AiState`. `prompt.rs` builds the user message and parses responses keyed by item index. Default system prompt uses a `{{language}}` placeholder.

- **Control-code masking** (`ai/codes.rs`): control-code preservation is enforced in code, not left to the prompt. Before sending, RPG Maker escape codes (`\C[2]`, `\N[1]`, `\{`, `%1`, …) are replaced with `⟦n⟧` placeholders (runs of adjacent codes collapse into one). Formatting codes at the start or end of a string are stripped and reattached verbatim; content codes (`\V \N \P \G`) that may move within the sentence always become placeholders. Responses are unmasked and validated, and when the `require_codes` setting is on, translations with missing or duplicated placeholders count as failed. Items with nothing but codes are skipped (`AiSummary.skipped`).
- **Dialogue block merging** (`ai/lines.rs`, `runner.rs::build_jobs`): the frontend sets `AiItem.merge` only when every dialogue (401) line of a group is in the request. Such lines are joined into one job, translated, then split at spaces back into at most the original line count. Empty strings fill the remaining lines, because 401 commands can't be added. The split uses the fewest lines that fit `max_line_width`, measured by `codes::display_width`, and balances line lengths. It is disabled for targets that don't use spaces (Japanese/Chinese/Thai).
- **Glossary**: `GlossaryTerm`s are stored in `project.json` (`store.rs`, not the AI settings). `ai/glossary.rs` includes only the terms that occur in a batch in that batch's prompt. `src/lib/glossary.ts` highlights terms in the UI. Both use the same matching rules (substring match with no word boundaries, for CJK; longest term wins; the first duplicate source wins), so keep them in sync.

### Frontend ↔ backend contract
`src/types.ts` mirrors Rust serde types (`model.rs`, `store.rs`, `ai/*`) with camelCase renaming — update both sides together. Tauri command wrappers live in `src/api/`; new commands must also be registered in `src-tauri/src/lib.rs` `generate_handler!`. `Kind` additions need a label in `KIND_LABELS`; plugin kinds and `jsonData` are flagged as risky in the UI (warning badge, excluded from AI by default, export warning) via `isRiskyKind`.
