import { useState } from "react";
import { useProject } from "../../stores/projectStore";
import type { ArgCommand, TextRule } from "../../types";
import { Button, Dialog, inputClass } from "../ui";

/** 편집 중인 규칙. 접두와 인자 명령은 입력란 그대로의 문자열로 들고 있다가 적용할 때 나눈다 */
interface Row {
  key: number;
  pattern: string;
  /** 공백으로 구분한 접두 */
  prefixes: string;
  /** 한 줄에 하나씩 "명령 인자번호" */
  args: string;
}

let nextKey = 0;

const toRow = (r: TextRule): Row => ({
  key: nextKey++,
  pattern: r.pattern,
  prefixes: r.skipPrefixes.join(" "),
  args: r.argCommands.map((a) => `${a.command} ${a.arg}`).join("\n"),
});

/** 입력한 경로를 게임 루트 기준 상대 경로('/' 구분)로 정리한다 */
const normalize = (p: string) => p.trim().replace(/\\/g, "/").replace(/^(\.\/|\/)+/, "");

/** 게임 자체 스크립트 등 텍스트 파일의 추출 규칙. 적용하면 아이템 목록을 다시 추출한다. */
export function TextRulesDialog({ onApply, onClose }: { onApply: (rules: TextRule[]) => void; onClose: () => void }) {
  const [rows, setRows] = useState<Row[]>(() => useProject.getState().options.textRules.map(toRow));
  const [error, setError] = useState<string | null>(null);

  const edit = (key: number, changes: Partial<Row>) => {
    setRows((rows) => rows.map((r) => (r.key === key ? { ...r, ...changes } : r)));
    setError(null);
  };

  const apply = () => {
    const rules: TextRule[] = [];
    for (const r of rows) {
      const pattern = normalize(r.pattern);
      if (!pattern) continue;
      const argCommands: ArgCommand[] = [];
      for (const line of r.args.split("\n").map((l) => l.trim()).filter(Boolean)) {
        const m = /^(\S+)\s+(\d+)$/.exec(line);
        if (!m || Number(m[2]) < 1) return setError(`인자 명령은 "명령 인자번호" 형식으로 입력하세요 (번호는 1부터): ${line}`);
        argCommands.push({ command: m[1], arg: Number(m[2]) });
      }
      rules.push({ pattern, skipPrefixes: r.prefixes.split(/\s+/).filter(Boolean), argCommands });
    }
    onApply(rules);
  };

  return (
    <Dialog
      title="텍스트 파일"
      width="w-[760px]"
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
        게임이 자체 스크립트 텍스트 파일(예: <code>Data/Text_Script/*.txt</code>)에 대사를 두는 경우, 파일 패턴과 줄 규칙을 등록하세요.
      </p>
      <ul className="mb-3 list-disc space-y-1 pl-5 text-xs text-zinc-400">
        <li>명령·주석 접두로 시작하지 않는 줄이 이어진 묶음(빈 줄이나 명령 줄로 끝남)이 항목 하나가 됩니다. 바로 앞 명령 줄은 맥락으로 표시됩니다.</li>
        <li>
          인자 명령: 그 명령 줄을 공백으로 나눴을 때 지정한 번째(명령 자신이 0) 토큰을 번역합니다. 예: <code>@putselect 1</code>,{" "}
          <code>@select 3</code>. 번역문의 공백은 인자가 나뉘지 않도록 내보낼 때 전각 공백으로 바뀝니다.
        </li>
        <li>번역문에 빈 줄이나 명령 접두로 시작하는 줄이 있으면 스크립트 구조가 깨지므로 내보낼 때 빈 줄은 빼고, 접두로 시작하는 줄이 있으면 적용하지 않습니다.</li>
        <li>파일 패턴은 게임 루트 기준이고 <code>*</code>는 폴더 안, <code>**</code>는 하위 폴더까지 포함합니다. 대소문자는 구분하지 않습니다.</li>
      </ul>

      <div className="space-y-3">
        {rows.map((r) => (
          <div key={r.key} className="space-y-2 rounded border border-zinc-700 p-3">
            <div className="flex items-center gap-2">
              <span className="w-24 shrink-0 text-xs text-zinc-400">파일 패턴</span>
              <input
                className={`${inputClass} flex-1 font-mono`}
                placeholder="Data/Text_Script/**/*.txt"
                value={r.pattern}
                onChange={(e) => edit(r.key, { pattern: e.target.value })}
              />
              <Button variant="ghost" onClick={() => setRows(rows.filter((x) => x.key !== r.key))} title="규칙 삭제">
                ✕
              </Button>
            </div>
            <div className="flex items-center gap-2">
              <span className="w-24 shrink-0 text-xs text-zinc-400">명령·주석 접두</span>
              <input
                className={`${inputClass} flex-1 font-mono`}
                placeholder="@ # ::"
                value={r.prefixes}
                onChange={(e) => edit(r.key, { prefixes: e.target.value })}
              />
            </div>
            <div className="flex items-start gap-2">
              <span className="w-24 shrink-0 pt-1 text-xs text-zinc-400">인자 명령</span>
              <textarea
                className={`${inputClass} h-20 flex-1 font-mono`}
                placeholder={"@putselect 1\n@select 3"}
                value={r.args}
                onChange={(e) => edit(r.key, { args: e.target.value })}
              />
            </div>
          </div>
        ))}
      </div>
      {rows.length === 0 && <p className="mb-2 text-sm text-zinc-500">등록된 규칙이 없습니다.</p>}
      <Button
        className="mt-3"
        onClick={() => setRows([...rows, { key: nextKey++, pattern: "", prefixes: "@ # ::", args: "" }])}
      >
        + 규칙 추가
      </Button>
      {error && <p className="mt-3 text-sm text-rose-300">{error}</p>}
    </Dialog>
  );
}
