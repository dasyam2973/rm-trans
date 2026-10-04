import { useMemo, useRef, type MouseEvent } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { filterAssets, fileNameOf, folderOf, formatSize, useAssets, type CryptFilter } from "../../stores/assetStore";
import { useProject } from "../../stores/projectStore";
import { ASSET_KIND_LABELS, SCHEME_LABELS, type AssetFile, type AssetKind } from "../../types";
import { Button, inputClass } from "../ui";
import { AssetPreview } from "./AssetPreview";
import { AssetSidebar } from "./AssetSidebar";

const CRYPT_LABELS: Record<CryptFilter, string> = { all: "전체", encrypted: "암호화", plain: "평문" };

export function AssetView({ onKeys }: { onKeys: () => void }) {
  const { scan, loading, error, replaced, folder, query, kinds, crypt, replacedOnly, selected, preview } = useAssets();
  const userKeys = useProject((s) => s.assetKeys);
  const files = scan?.files;
  const filtered = useMemo(
    () => (files ? filterAssets(files, replaced, { folder, query, kinds, crypt, replacedOnly }) : []),
    [files, replaced, folder, query, kinds, crypt, replacedOnly],
  );
  const order = useRef<string[]>([]);
  order.current = useMemo(() => filtered.map((f) => f.path), [filtered]);
  const presentKinds = useMemo(() => {
    const present = new Set(files?.map((f) => f.kind));
    return (Object.keys(ASSET_KIND_LABELS) as AssetKind[]).filter((k) => present.has(k));
  }, [files]);
  // 키가 없어 복호화할 수 없는 방식
  const missingKeys = useMemo(
    () => scan?.schemes.filter((x) => !(userKeys[x.scheme] ?? x.key)) ?? [],
    [scan, userKeys],
  );

  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 30,
    getItemKey: (i) => filtered[i].path,
    overscan: 12,
  });

  if (loading) return <div className="flex flex-1 items-center justify-center text-zinc-500">리소스를 찾는 중…</div>;
  if (error) return <div className="flex flex-1 items-center justify-center p-8 text-rose-400">{error}</div>;
  if (!scan) return <div className="flex-1" />;

  const s = useAssets.getState();
  const toggleKind = (k: AssetKind) => s.set({ kinds: kinds.includes(k) ? kinds.filter((x) => x !== k) : [...kinds, k] });
  const selectedInView = filtered.filter((f) => selected.has(f.path)).length;
  const allInView = filtered.length > 0 && selectedInView === filtered.length;
  const onCheck = (path: string, e: MouseEvent) => {
    if (e.shiftKey) s.selectRange(order.current, path);
    else s.toggle(path);
  };

  const notices = [
    ...scan.warnings,
    ...missingKeys.map((x) => `${SCHEME_LABELS[x.scheme]} 방식의 암호화 키를 찾지 못했습니다. 키를 직접 입력해 주세요.`),
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {notices.length > 0 && (
        <div className="flex items-start gap-2 border-b border-amber-900 bg-amber-950/50 px-3 py-1.5 text-sm text-amber-200">
          <span className="flex-1 whitespace-pre-wrap">{notices.join("\n")}</span>
          <Button onClick={onKeys}>암호화 키</Button>
        </div>
      )}
      <div className="flex flex-wrap items-center gap-2 border-b border-zinc-800 bg-zinc-900/60 px-3 py-2">
        <input
          className={`${inputClass} w-72`}
          placeholder="경로 검색"
          value={query}
          onChange={(e) => s.set({ query: e.target.value })}
        />
        <div className="flex items-center gap-1">
          {presentKinds.map((k) => (
            <Chip key={k} on={kinds.includes(k)} onClick={() => toggleKind(k)}>
              {ASSET_KIND_LABELS[k]}
            </Chip>
          ))}
        </div>
        <div className="flex items-center gap-1">
          {(Object.keys(CRYPT_LABELS) as CryptFilter[]).map((c) => (
            <Chip key={c} on={crypt === c} onClick={() => s.set({ crypt: c })}>
              {CRYPT_LABELS[c]}
            </Chip>
          ))}
        </div>
        <Chip on={replacedOnly} onClick={() => s.set({ replacedOnly: !replacedOnly })}>
          {`번역 이미지 (${replaced.size})`}
        </Chip>
        <span className="ml-auto text-xs text-zinc-400">
          {filtered.length.toLocaleString()} / {scan.files.length.toLocaleString()}개
          {selected.size > 0 && ` · 선택 ${selected.size.toLocaleString()}개`}
        </span>
        {selected.size > 0 && (
          <Button variant="ghost" onClick={() => s.setMany([...selected], false)}>
            선택 해제
          </Button>
        )}
      </div>

      <div className="flex min-h-0 flex-1">
        <AssetSidebar files={scan.files} webDir={scan.webDir} />
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="grid grid-cols-[28px_1fr_72px_150px_80px] items-center gap-2 border-b border-zinc-800 bg-zinc-900 py-1.5 pr-3 pl-2 text-xs text-zinc-400">
            <input
              type="checkbox"
              className="accent-sky-500"
              checked={allInView}
              ref={(el) => {
                if (el) el.indeterminate = selectedInView > 0 && !allInView;
              }}
              onChange={() => s.setMany(order.current, !allInView)}
              title="목록 전체 선택"
            />
            <span>파일</span>
            <span>종류</span>
            <span>암호화</span>
            <span className="text-right">크기</span>
          </div>
          <div ref={scrollRef} className="flex-1 overflow-y-auto">
            {filtered.length === 0 ? (
              <div className="p-8 text-center text-zinc-500">표시할 리소스가 없습니다.</div>
            ) : (
              <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
                {virtualizer.getVirtualItems().map((v) => {
                  const file = filtered[v.index];
                  return (
                    <AssetRow
                      key={v.key}
                      file={file}
                      top={v.start}
                      showFolder={folder === null}
                      checked={selected.has(file.path)}
                      active={preview === file.path}
                      replaced={replaced.has(file.plainPath)}
                      onCheck={onCheck}
                    />
                  );
                })}
              </div>
            )}
          </div>
        </div>
        <AssetPreview />
      </div>
    </div>
  );
}

function AssetRow({
  file,
  top,
  showFolder,
  checked,
  active,
  replaced,
  onCheck,
}: {
  file: AssetFile;
  top: number;
  showFolder: boolean;
  checked: boolean;
  active: boolean;
  replaced: boolean;
  onCheck: (path: string, e: MouseEvent) => void;
}) {
  return (
    <div
      onClick={() => useAssets.getState().set({ preview: file.path })}
      className={`absolute top-0 left-0 grid h-7.5 w-full cursor-default grid-cols-[28px_1fr_72px_150px_80px] items-center gap-2 border-b border-zinc-800/60 pr-3 pl-2 text-sm ${
        active ? "bg-sky-600/20" : checked ? "bg-zinc-800/60" : "hover:bg-zinc-900"
      }`}
      style={{ transform: `translateY(${top}px)` }}
    >
      <input
        type="checkbox"
        className="accent-sky-500"
        checked={checked}
        onClick={(e) => {
          e.stopPropagation();
          onCheck(file.path, e);
        }}
        readOnly
      />
      <span className="truncate" title={file.path}>
        {replaced && (
          <span className="mr-1.5 rounded bg-emerald-600/30 px-1 text-xs text-emerald-300" title="번역 이미지 등록됨">
            번역
          </span>
        )}
        {fileNameOf(file.plainPath)}
        {showFolder && <span className="ml-2 text-xs text-zinc-500">{folderOf(file.path)}</span>}
      </span>
      <span className="text-xs text-zinc-400">{ASSET_KIND_LABELS[file.kind]}</span>
      <SchemeBadge file={file} />
      <span className="text-right text-xs text-zinc-500">{formatSize(file.size)}</span>
    </div>
  );
}

export function SchemeBadge({ file }: { file: AssetFile }) {
  if (!file.encrypted) return <span className="text-xs text-zinc-600">평문</span>;
  if (!file.scheme) return <span className="text-xs text-rose-400">⚠ 알 수 없는 방식</span>;
  return <span className="truncate text-xs text-sky-300">{SCHEME_LABELS[file.scheme]}</span>;
}

function Chip({ on, onClick, children }: { on: boolean; onClick: () => void; children: string }) {
  return (
    <button
      onClick={onClick}
      className={`rounded border px-2 py-0.5 text-xs ${
        on ? "border-sky-500 bg-sky-600/30 text-sky-200" : "border-zinc-700 text-zinc-400 hover:bg-zinc-800"
      }`}
    >
      {children}
    </button>
  );
}
