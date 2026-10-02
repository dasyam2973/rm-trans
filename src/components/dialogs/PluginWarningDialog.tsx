import { useState } from "react";
import { Button, Dialog } from "../ui";

/** 플러그인 데이터 추출을 켜기 전에 위험성을 알리고 확인받는다. */
export function PluginWarningDialog({ onConfirm, onClose }: { onConfirm: () => void; onClose: () => void }) {
  const [agreed, setAgreed] = useState(false);

  return (
    <Dialog
      title="플러그인 데이터 추출"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            취소
          </Button>
          <Button variant="danger" onClick={onConfirm} disabled={!agreed}>
            플러그인 데이터 포함
          </Button>
        </>
      }
    >
      <div className="mb-3 rounded border border-amber-600/60 bg-amber-950/40 px-3 py-2 text-sm text-amber-200">
        ⚠ 플러그인 데이터에는 화면에 표시되는 문구뿐 아니라 플러그인이 동작하는 데 필요한 설정값이 섞여 있습니다. 잘못
        수정하면 게임이 실행되지 않거나, 특정 상황에서 오류가 나거나, 기능이 조용히 동작하지 않을 수 있습니다.
      </div>
      <ul className="mb-3 list-disc space-y-1 pl-5 text-sm text-zinc-300">
        <li>
          이미지·효과음 <b>파일명</b>, 스위치·변수 이름, 커맨드·태그 이름, <b>스크립트 코드</b>, 색상·좌표 같은 값은 번역하면 안
          됩니다.
        </li>
        <li>화면에 실제로 표시되는 문구인지 확인한 뒤 번역하고, 확실하지 않으면 원문 그대로 두세요.</li>
        <li>AI 번역은 이런 식별자까지 번역할 수 있으므로 결과를 꼭 직접 확인하세요.</li>
        <li>내보낸 뒤 반드시 게임을 실행해 확인하고, 문제가 생기면 해당 항목을 원문으로 되돌리세요.</li>
      </ul>
      <p className="mb-3 text-xs text-zinc-500">
        추출 대상: js/plugins.js의 켜져 있는 플러그인 파라미터, MZ 이벤트의 플러그인 커맨드 인자. MV 플러그인 커맨드는 명령 문자열
        전체가 한 줄이라 일부만 번역하면 명령이 깨지므로 추출하지 않습니다. 나중에 옵션을 끄더라도 입력한 번역은 작업 파일에
        보존되지만 내보내기에는 적용되지 않습니다.
      </p>
      <label className="flex items-center gap-2 text-sm">
        <input type="checkbox" className="accent-rose-500" checked={agreed} onChange={(e) => setAgreed(e.target.checked)} />
        위험성을 이해했으며, 수정한 내용은 직접 확인하겠습니다.
      </label>
    </Dialog>
  );
}
