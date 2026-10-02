import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "default" | "primary" | "ghost" | "danger";

const variants: Record<Variant, string> = {
  default: "bg-zinc-800 hover:bg-zinc-700 text-zinc-100 border border-zinc-700",
  primary: "bg-sky-600 hover:bg-sky-500 text-white border border-sky-500",
  ghost: "hover:bg-zinc-800 text-zinc-300 border border-transparent",
  danger: "bg-rose-700 hover:bg-rose-600 text-white border border-rose-600",
};

export function Button({
  variant = "default",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      className={`inline-flex items-center gap-1.5 rounded px-2.5 py-1 text-sm whitespace-nowrap transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${variants[variant]} ${className}`}
      {...props}
    />
  );
}

/** 켜고 끄는 작은 토글 버튼 (정규식, 대소문자 등) */
export function Toggle({
  on,
  onChange,
  title,
  children,
}: {
  on: boolean;
  onChange: (v: boolean) => void;
  title: string;
  children: ReactNode;
}) {
  return (
    <button
      title={title}
      onClick={() => onChange(!on)}
      className={`rounded px-1.5 py-0.5 font-mono text-xs border ${
        on ? "bg-sky-600/30 border-sky-500 text-sky-200" : "border-zinc-700 text-zinc-400 hover:bg-zinc-800"
      }`}
    >
      {children}
    </button>
  );
}

export const inputClass =
  "rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-sm text-zinc-100 outline-none focus:border-sky-500";

export function Dialog({
  title,
  onClose,
  children,
  footer,
  width = "w-[560px]",
}: {
  title: string;
  onClose?: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: string;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onMouseDown={onClose}>
      <div
        className={`${width} max-h-[90vh] flex flex-col rounded-lg border border-zinc-700 bg-zinc-900 shadow-2xl`}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-zinc-800 px-4 py-2.5">
          <h2 className="font-semibold">{title}</h2>
          {onClose && (
            <button className="text-zinc-500 hover:text-zinc-200" onClick={onClose}>
              ✕
            </button>
          )}
        </div>
        <div className="overflow-y-auto p-4">{children}</div>
        {footer && <div className="flex justify-end gap-2 border-t border-zinc-800 px-4 py-2.5">{footer}</div>}
      </div>
    </div>
  );
}

export function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="mb-3 flex flex-col gap-1">
      <span className="text-xs text-zinc-400">{label}</span>
      {children}
      {hint && <span className="text-xs text-zinc-500">{hint}</span>}
    </label>
  );
}
