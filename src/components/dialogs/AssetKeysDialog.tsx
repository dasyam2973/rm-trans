import { useState } from "react";
import { deriveAssetKey } from "../../api/assets";
import { useAssets } from "../../stores/assetStore";
import { useProject } from "../../stores/projectStore";
import { SCHEME_KEY_BYTES, SCHEME_LABELS, type KeySource, type SchemeInfo } from "../../types";
import { Button, Dialog, inputClass } from "../ui";

const SOURCE_LABELS: Record<KeySource, string> = {
  system: "System.json에서 찾음",
  recovered: "이미지에서 복구함",
  none: "찾지 못함",
};

type InputMode = "system" | "raw";

/** 암호화 방식별 키 확인 및 직접 입력 */
export function AssetKeysDialog({ onClose }: { onClose: () => void }) {
  const scan = useAssets((s) => s.scan);
  if (!scan) return null;
  const unknown = scan.files.filter((f) => f.encrypted && !f.scheme).length;

  return (
    <Dialog title="암호화 키" onClose={onClose} width="w-[640px]" footer={<Button onClick={onClose}>닫기</Button>}>
      <dl className="mb-4 grid grid-cols-[140px_1fr] gap-x-3 gap-y-1 text-sm">
        <dt className="text-zinc-500">System.json 키</dt>
        <dd className="font-mono break-all text-zinc-300">{scan.systemKey ?? "없음"}</dd>
        {scan.cryptoPlugins.length > 0 && (
          <>
            <dt className="text-zinc-500">암호화 플러그인</dt>
            <dd className="text-zinc-300">{scan.cryptoPlugins.join(", ")}</dd>
          </>
        )}
      </dl>
      {scan.schemes.length === 0 && unknown === 0 && <p className="text-sm text-zinc-400">암호화된 리소스가 없습니다.</p>}
      {scan.schemes.map((info) => (
        <SchemeKeyRow key={info.scheme} info={info} />
      ))}
      {unknown > 0 && (
        <p className="rounded border border-rose-800/60 bg-rose-950/40 px-3 py-2 text-sm text-rose-200">
          암호화 방식을 알 수 없는 파일이 {unknown.toLocaleString()}개 있습니다. 지원하지 않는 암호화 플러그인을 쓰는 게임일 수
          있습니다.
        </p>
      )}
    </Dialog>
  );
}

function SchemeKeyRow({ info }: { info: SchemeInfo }) {
  const userKey = useProject((s) => s.assetKeys[info.scheme]);
  const [mode, setMode] = useState<InputMode>("system");
  const [input, setInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const bytes = SCHEME_KEY_BYTES[info.scheme];

  const apply = async () => {
    setError(null);
    const value = input.trim();
    try {
      let key: string;
      if (mode === "system") {
        key = await deriveAssetKey(info.scheme, value);
      } else {
        if (!new RegExp(`^[0-9a-fA-F]{${bytes * 2}}$`).test(value)) {
          throw new Error(`실제 키는 ${bytes * 2}자리 hex여야 합니다.`);
        }
        key = value.toLowerCase();
      }
      useProject.getState().setAssetKey(info.scheme, key);
      setInput("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <section className="mb-4 rounded border border-zinc-800 p-3">
      <h3 className="mb-2 flex items-baseline justify-between text-sm font-semibold">
        {SCHEME_LABELS[info.scheme]}
        <span className="text-xs font-normal text-zinc-500">{info.fileCount.toLocaleString()}개 파일</span>
      </h3>
      <dl className="mb-3 grid grid-cols-[80px_1fr] gap-x-3 gap-y-1 text-xs">
        <dt className="text-zinc-500">감지한 키</dt>
        <dd className={`font-mono break-all ${userKey ? "text-zinc-500 line-through" : "text-zinc-200"}`}>
          {info.key ?? "-"} <span className="font-sans text-zinc-500 no-underline">({SOURCE_LABELS[info.keySource]})</span>
        </dd>
        {userKey && (
          <>
            <dt className="text-zinc-500">입력한 키</dt>
            <dd className="flex items-center gap-2 font-mono break-all text-sky-300">
              {userKey}
              <button
                className="font-sans text-zinc-500 hover:text-zinc-200"
                onClick={() => useProject.getState().setAssetKey(info.scheme, null)}
              >
                되돌리기
              </button>
            </dd>
          </>
        )}
      </dl>
      {info.keyMismatch && (
        <p className="mb-3 text-xs text-amber-300">
          System.json의 키가 실제 파일과 맞지 않아, 이미지에서 복구한 키를 사용합니다.
        </p>
      )}
      <div className="flex gap-2">
        <select className={inputClass} value={mode} onChange={(e) => setMode(e.target.value as InputMode)}>
          <option value="system">System.json 형식</option>
          <option value="raw">실제 키 (hex)</option>
        </select>
        <input
          className={`${inputClass} flex-1 font-mono`}
          placeholder={mode === "system" ? "encryptionKey 값" : `${bytes * 2}자리 hex`}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && input.trim() && apply()}
        />
        <Button onClick={apply} disabled={!input.trim()}>
          적용
        </Button>
      </div>
      {error && <p className="mt-1 text-xs text-rose-400">{error}</p>}
    </section>
  );
}
