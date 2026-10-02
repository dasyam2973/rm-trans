import { useEffect, useState } from "react";
import { defaultSystemPrompt, getAiSettings, saveAiSettings } from "../../api/ai";
import { RESPONSE_MODE_LABELS, TARGET_LANGUAGES, type AiSettings, type ResponseMode } from "../../types";
import { Button, Dialog, Field, inputClass } from "../ui";

export function AiSettingsDialog({ onClose }: { onClose: () => void }) {
  const [s, setS] = useState<AiSettings | null>(null);
  const [showKey, setShowKey] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getAiSettings().then(setS, (e) => setError(String(e)));
  }, []);

  const patch = (p: Partial<AiSettings>) => setS((prev) => (prev ? { ...prev, ...p } : prev));

  const save = async () => {
    if (!s) return;
    try {
      await saveAiSettings(s);
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <Dialog
      title="AI 설정"
      width="w-[680px]"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            취소
          </Button>
          <Button variant="primary" onClick={save} disabled={!s}>
            저장
          </Button>
        </>
      }
    >
      {!s ? (
        <p className="text-sm text-zinc-400">{error ?? "불러오는 중…"}</p>
      ) : (
        <>
          <p className="mb-4 text-xs text-zinc-500">
            OpenAI 호환 Chat Completions API(/chat/completions)라면 어떤 서비스든 사용할 수 있습니다. API 키는 이 PC의 앱
            설정 폴더에 평문으로 저장됩니다.
          </p>
          <Field label="API Base URL" hint="예: https://api.openai.com/v1, https://openrouter.ai/api/v1, http://localhost:11434/v1">
            <input className={inputClass} value={s.baseUrl} onChange={(e) => patch({ baseUrl: e.target.value })} />
          </Field>
          <Field label="API 키" hint="키가 필요 없는 로컬 서버라면 비워 두세요.">
            <div className="flex gap-2">
              <input
                className={`${inputClass} flex-1 font-mono`}
                type={showKey ? "text" : "password"}
                value={s.apiKey}
                onChange={(e) => patch({ apiKey: e.target.value })}
              />
              <Button variant="ghost" onClick={() => setShowKey(!showKey)}>
                {showKey ? "숨기기" : "보기"}
              </Button>
            </div>
          </Field>
          <Field label="모델">
            <input
              className={inputClass}
              value={s.model}
              placeholder="모델 이름"
              onChange={(e) => patch({ model: e.target.value })}
            />
          </Field>
          <div className="grid grid-cols-3 gap-3">
            <Field label="Temperature" hint="비우면 API 기본값">
              <input
                className={inputClass}
                type="number"
                step="0.1"
                min="0"
                max="2"
                value={s.temperature ?? ""}
                onChange={(e) => patch({ temperature: e.target.value === "" ? null : Number(e.target.value) })}
              />
            </Field>
            <Field label="요청당 항목 수">
              <input
                className={inputClass}
                type="number"
                min="1"
                value={s.batchSize}
                onChange={(e) => patch({ batchSize: Math.max(1, Number(e.target.value)) })}
              />
            </Field>
            <Field label="동시 요청 수">
              <input
                className={inputClass}
                type="number"
                min="1"
                value={s.concurrency}
                onChange={(e) => patch({ concurrency: Math.max(1, Number(e.target.value)) })}
              />
            </Field>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <Field label="번역 대상 언어">
              <select className={inputClass} value={s.targetLanguage} onChange={(e) => patch({ targetLanguage: e.target.value })}>
                {!TARGET_LANGUAGES.some((l) => l.value === s.targetLanguage) && (
                  <option value={s.targetLanguage}>{s.targetLanguage}</option>
                )}
                {TARGET_LANGUAGES.map((l) => (
                  <option key={l.value} value={l.value}>
                    {l.label}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="응답 형식" hint="API가 지원하는 방식을 고르세요.">
              <select
                className={inputClass}
                value={s.responseMode}
                onChange={(e) => patch({ responseMode: e.target.value as ResponseMode })}
              >
                {(Object.keys(RESPONSE_MODE_LABELS) as ResponseMode[]).map((m) => (
                  <option key={m} value={m}>
                    {RESPONSE_MODE_LABELS[m]}
                  </option>
                ))}
              </select>
            </Field>
          </div>
          <Field
            label="시스템 프롬프트"
            hint="번역 방침을 자유롭게 적으세요. {{language}}는 대상 언어로 치환되고, 입출력 JSON 형식 지시는 자동으로 뒤에 붙습니다."
          >
            <textarea
              className={`${inputClass} h-48 resize-y font-mono text-xs`}
              value={s.systemPrompt}
              onChange={(e) => patch({ systemPrompt: e.target.value })}
            />
          </Field>
          <Button variant="ghost" onClick={async () => patch({ systemPrompt: await defaultSystemPrompt() })}>
            기본 프롬프트로 되돌리기
          </Button>
          {error && <p className="mt-2 text-sm text-rose-400">{error}</p>}
        </>
      )}
    </Dialog>
  );
}
