import { useState } from "react";
import { Button, Dialog } from "../ui";

/** 세부 수정 모드를 켜기 전에 위험성을 알리고 확인받는다. */
export function DetailedWarningDialog({ onConfirm, onClose }: { onConfirm: () => void; onClose: () => void }) {
  const [agreed, setAgreed] = useState(false);

  return (
    <Dialog
      title="세부 수정"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            취소
          </Button>
          <Button variant="danger" onClick={onConfirm} disabled={!agreed}>
            세부 수정 켜기
          </Button>
        </>
      }
    >
      <div className="mb-3 rounded border border-amber-600/60 bg-amber-950/40 px-3 py-2 text-sm text-amber-200">
        ⚠ 세부 수정은 번역 문구가 아닌 설정값까지 목록에 보여 줍니다. 잘못 수정하면 게임이 실행되지 않거나, 특정 상황에서 오류가
        나거나, 기능이 조용히 동작하지 않을 수 있습니다.
      </div>
      <ul className="mb-3 list-disc space-y-1 pl-5 text-sm text-zinc-300">
        <li>
          <b>외부 JSON 파일</b>: 게임 폴더 아래의 JSON 파일 중 RPG Maker 기본 데이터가 아닌 파일(플러그인 전용 데이터 등)의 문자열
          값을 모두 추출합니다. package.json은 제외합니다.
        </li>
        <li>
          <b>플러그인 설정값</b>: 플러그인 데이터를 포함한 경우, 텍스트가 아니어서 걸러내던 숫자·불리언 같은 값도 추출합니다.
        </li>
        <li>어떤 값이 화면에 표시되는 문구인지 알 수 없으므로, 확실하지 않으면 원문 그대로 두세요.</li>
        <li>값의 형식(숫자, true/false 등)을 바꾸지 않도록 주의하고, 내보낸 뒤 반드시 게임을 실행해 확인하세요.</li>
      </ul>
      <p className="mb-3 text-xs text-zinc-500">
        문자열로 저장된 값만 수정할 수 있습니다. 나중에 옵션을 끄더라도 입력한 내용은 작업 파일에 보존되지만 내보내기에는 적용되지
        않습니다.
      </p>
      <label className="flex items-center gap-2 text-sm">
        <input type="checkbox" className="accent-rose-500" checked={agreed} onChange={(e) => setAgreed(e.target.checked)} />
        위험성을 이해했으며, 수정한 내용은 직접 확인하겠습니다.
      </label>
    </Dialog>
  );
}
