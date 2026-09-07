import { useState } from "react";
import { copyToClipboard } from "../lib/clipboard";

type Props = {
  text: string;
  label?: string;
  className?: string;
};

/**
 * One-click copy control (works on HTTP LAN hosts, not only localhost/HTTPS).
 */
export default function CopyButton({ text, label = "复制", className = "btn btn-ghost btn-sm" }: Props) {
  const [done, setDone] = useState(false);
  const [failed, setFailed] = useState(false);

  const handleCopy = async () => {
    const ok = await copyToClipboard(text);
    if (ok) {
      setFailed(false);
      setDone(true);
      setTimeout(() => setDone(false), 1600);
    } else {
      setDone(false);
      setFailed(true);
      setTimeout(() => setFailed(false), 2200);
    }
  };

  return (
    <button type="button" className={className} onClick={handleCopy} title={failed ? "复制失败" : undefined}>
      <svg viewBox="0 0 24 24" width="14" height="14">
        <rect x="9" y="9" width="11" height="11" rx="2" />
        <path d="M5 15V5a2 2 0 0 1 2-2h10" />
      </svg>
      {failed ? "复制失败" : done ? "已复制" : label}
    </button>
  );
}
