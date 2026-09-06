import { useState } from "react";

type Props = {
  text: string;
  label?: string;
  className?: string;
};

/**
 * One-click copy control (common MCP / API key UX).
 */
export default function CopyButton({ text, label = "复制", className = "btn btn-ghost btn-sm" }: Props) {
  const [done, setDone] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setDone(true);
      setTimeout(() => setDone(false), 1600);
    } catch {
      /* ignore */
    }
  };

  return (
    <button type="button" className={className} onClick={handleCopy}>
      <svg viewBox="0 0 24 24" width="14" height="14">
        <rect x="9" y="9" width="11" height="11" rx="2" />
        <path d="M5 15V5a2 2 0 0 1 2-2h10" />
      </svg>
      {done ? "已复制" : label}
    </button>
  );
}
