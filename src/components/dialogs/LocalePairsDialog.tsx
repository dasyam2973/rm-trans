import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useProject } from "../../stores/projectStore";
import type { LocalePair } from "../../types";
import { Button, Dialog, inputClass } from "../ui";

interface Row extends LocalePair {
  key: number;
}

let nextKey = 0;

/** 입력한 경로를 게임 루트 기준 상대 경로('/' 구분)로 정리한다 */
const normalize = (p: string) => p.trim().replace(/\\/g, "/").replace(/^(\.\/|\/)+/, "");

/** 번역 플러그인의 언어 파일 쌍 설정. 적용하면 아이템 목록을 다시 추출한다. */
export function LocalePairsDialog({ onApply, onClose }: { onApply: (pairs: LocalePair[]) => void; onClose: () => void }) {
  const root = useProject((s) => s.project!.root);
  const [rows, setRows] = useState<Row[]>(() =>
    useProject.getState().options.localePairs.map((p) => ({ ...p, key: nextKey++ })),
  );
  const [error, setError] = useState<string | null>(null);

  const edit = (key: number, changes: Partial<LocalePair>) => {
    setRows((rows) => rows.map((r) => (r.key === key ? { ...r, ...changes } : r)));
    setError(null);
  };

  /** 게임 폴더 안의 JSON 파일을 골라 상대 경로로 돌려준다 */
  const pick = async (): Promise<string | null> => {
    const file = await open({ defaultPath: root, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (typeof file !== "string") return null;
    const base = normalize(root).replace(/\/+$/, "") + "/";
    const abs = normalize(file);
    if (!abs.toLowerCase().startsWith(base.toLowerCase())) {
      setError("게임 폴더 안의 파일을 선택하세요.");
      return null;
    }
    return abs.slice(base.length);
  };

  const pickSource = async (row: Row) => {
    const source = await pick();
    if (source === null) return;
    // 대상이 비어 있으면 같은 폴더를 미리 채워 둔다 (파일 이름만 입력하면 되도록)
    const dir = source.includes("/") ? source.slice(0, source.lastIndexOf("/") + 1) : "";
    edit(row.key, { source, target: row.target || dir });
  };

  const pickTarget = async (row: Row) => {
    const target = await pick();
    if (target !== null) edit(row.key, { target });
  };

  const apply = () => {
    const pairs = rows
      .map((r) => ({ source: normalize(r.source), target: normalize(r.target) }))
      .filter((p) => p.source || p.target);
    const targets = new Set<string>();
    for (const p of pairs) {
      if (!p.source.toLowerCase().endsWith(".json") || !p.target.toLowerCase().endsWith(".json")) {
        return setError(`JSON 파일 경로를 입력하세요: ${p.source || "(원본 없음)"} → ${p.target || "(대상 없음)"}`);
      }
      if (p.source === p.target) return setError(`원본과 대상이 같습니다: ${p.source}`);
      if (targets.has(p.target)) return setError(`대상 파일이 중복됩니다: ${p.target}`);
      targets.add(p.target);
    }
    onApply(pairs);
  };

  return (
    <Dialog
      title="언어 파일"
      width="w-[720px]"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            취소
          </Button>
          <Button variant="primary" onClick={apply}>
            적용
          </Button>
        </>
      }
    >
      <p className="mb-3 text-sm text-zinc-300">
        번역 플러그인이 언어별 JSON 파일(예: <code>ja.json</code>, <code>ko.json</code>)을 쓰는 경우, 원본 언어 파일과 번역할 대상
        파일을 짝지어 등록하세요.
      </p>
      <ul className="mb-3 list-disc space-y-1 pl-5 text-xs text-zinc-400">
        <li>원본 언어 파일의 항목으로 목록을 만들고, 대상 파일에 이미 있는 값은 번역으로 채워 둡니다.</li>
        <li>내보낼 때 대상 파일은 원본 언어 파일에 번역을 적용해 새로 만듭니다. 대상 파일에서 빠져 있던 항목은 원본 언어로 채워집니다.</li>
        <li>대상 파일에만 있고 원본 언어 파일에는 없는 항목은 내보낼 때 빠집니다.</li>
        <li>대상 파일이 아직 없으면 경로를 직접 입력하세요. 내보낼 때 새로 만듭니다.</li>
      </ul>

      <div className="mb-2 grid grid-cols-[1fr_auto_1fr_auto] items-center gap-2 text-xs text-zinc-400">
        <span>원본 언어 파일</span>
        <span />
        <span>대상 언어 파일</span>
        <span />
        {rows.map((r) => (
          <Pair
            key={r.key}
            row={r}
            onEdit={edit}
            onPickSource={pickSource}
            onPickTarget={pickTarget}
            onRemove={() => setRows(rows.filter((x) => x.key !== r.key))}
          />
        ))}
      </div>
      {rows.length === 0 && <p className="mb-2 text-sm text-zinc-500">등록된 언어 파일이 없습니다.</p>}
      <Button onClick={() => setRows([...rows, { key: nextKey++, source: "", target: "" }])}>+ 추가</Button>
      {error && <p className="mt-3 text-sm text-rose-300">{error}</p>}
    </Dialog>
  );
}

function Pair({
  row,
  onEdit,
  onPickSource,
  onPickTarget,
  onRemove,
}: {
  row: Row;
  onEdit: (key: number, changes: Partial<LocalePair>) => void;
  onPickSource: (row: Row) => void;
  onPickTarget: (row: Row) => void;
  onRemove: () => void;
}) {
  return (
    <>
      <div className="flex gap-1">
        <input
          className={`${inputClass} min-w-0 flex-1 font-mono`}
          placeholder="locales/ja.json"
          value={row.source}
          onChange={(e) => onEdit(row.key, { source: e.target.value })}
        />
        <Button onClick={() => onPickSource(row)}>…</Button>
      </div>
      <span className="text-zinc-500">→</span>
      <div className="flex gap-1">
        <input
          className={`${inputClass} min-w-0 flex-1 font-mono`}
          placeholder="locales/ko.json"
          value={row.target}
          onChange={(e) => onEdit(row.key, { target: e.target.value })}
        />
        <Button onClick={() => onPickTarget(row)}>…</Button>
      </div>
      <Button variant="ghost" onClick={onRemove} title="삭제">
        ✕
      </Button>
    </>
  );
}
