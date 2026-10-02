import { useCallback, useMemo, useRef, type MouseEvent } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { statusOf, translationOf } from "../../lib/status";
import { useProject } from "../../stores/projectStore";
import { useSelection } from "../../stores/selectionStore";
import type { Entry, Status } from "../../types";
import { EntryRow } from "./EntryRow";

type Row = { type: "group"; key: string; label: string; file: string; ids: string[] } | { type: "entry"; key: string; entry: Entry };

/** 연속된 같은 그룹의 아이템 앞에 그룹 헤더 행을 끼워 넣는다. */
function buildRows(entries: Entry[]): Row[] {
  const rows: Row[] = [];
  let header: Extract<Row, { type: "group" }> | null = null;
  for (const e of entries) {
    if (!header || header.key !== `g:${e.group}`) {
      header = { type: "group", key: `g:${e.group}`, label: e.groupLabel, file: e.file, ids: [] };
      rows.push(header);
    }
    header.ids.push(e.id);
    rows.push({ type: "entry", key: e.id, entry: e });
  }
  return rows;
}

export function EntryList({ filtered }: { filtered: Entry[] }) {
  const translations = useProject((s) => s.translations);
  const overrides = useProject((s) => s.overrides);
  const selected = useSelection((s) => s.selected);

  const rows = useMemo(() => buildRows(filtered), [filtered]);
  const order = useRef<string[]>([]);
  order.current = useMemo(() => filtered.map((e) => e.id), [filtered]);

  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => (rows[i].type === "group" ? 30 : 64),
    getItemKey: (i) => rows[i].key,
    overscan: 8,
  });

  const onSelect = useCallback((id: string, e: MouseEvent) => {
    const sel = useSelection.getState();
    if (e.shiftKey) sel.selectRange(order.current, id);
    else sel.toggle(id);
  }, []);
  const onCommit = useCallback((id: string, text: string) => useProject.getState().setTranslations({ [id]: text }), []);
  const onOverride = useCallback(
    (id: string, status: Status | null) => useProject.getState().setOverrides([id], status),
    [],
  );

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <div className="grid grid-cols-[28px_1fr_1fr_92px] gap-2 border-b border-zinc-800 bg-zinc-900 py-1.5 pr-3 pl-5 text-xs text-zinc-400">
        <span />
        <span>원문</span>
        <span>번역문</span>
        <span>상태</span>
      </div>
      <div ref={scrollRef} className="flex-1 overflow-y-auto">
        {rows.length === 0 ? (
          <div className="p-8 text-center text-zinc-500">표시할 항목이 없습니다.</div>
        ) : (
          <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
            {virtualizer.getVirtualItems().map((v) => {
              const row = rows[v.index];
              return (
                <div
                  key={v.key}
                  data-index={v.index}
                  ref={virtualizer.measureElement}
                  className="absolute top-0 left-0 w-full"
                  style={{ transform: `translateY(${v.start}px)` }}
                >
                  {row.type === "group" ? (
                    <GroupHeader row={row} selected={selected} />
                  ) : (
                    <EntryRow
                      entry={row.entry}
                      translation={translationOf(row.entry, translations)}
                      status={statusOf(row.entry, translations, overrides)}
                      override={overrides[row.entry.id]}
                      selected={selected.has(row.entry.id)}
                      onSelect={onSelect}
                      onCommit={onCommit}
                      onOverride={onOverride}
                    />
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function GroupHeader({ row, selected }: { row: Extract<Row, { type: "group" }>; selected: Set<string> }) {
  const count = row.ids.filter((id) => selected.has(id)).length;
  const all = count === row.ids.length;
  return (
    <div className="flex items-center gap-2 border-b border-zinc-800 bg-zinc-900/90 px-2 py-1.5 text-xs">
      <input
        type="checkbox"
        checked={all}
        ref={(el) => {
          if (el) el.indeterminate = count > 0 && !all;
        }}
        onChange={() => useSelection.getState().setMany(row.ids, !all)}
        className="accent-sky-500"
        title="그룹 전체 선택"
      />
      <span className="font-medium text-zinc-200">{row.label}</span>
      <span className="text-zinc-500">{row.file}</span>
      {row.ids.length > 1 && <span className="ml-auto text-zinc-500">{row.ids.length}개</span>}
    </div>
  );
}
