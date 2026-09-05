type Props = {
  title?: string;
  message: string;
  onClose?: () => void;
};

/** High-visibility page alert — sticky under the header area. */
export default function AlertBanner({ title = "操作失败", message, onClose }: Props) {
  if (!message.trim()) return null;
  return (
    <div className="alert-banner alert-error" role="alert">
      <div className="alert-banner-icon" aria-hidden>
        <svg viewBox="0 0 24 24" width="20" height="20">
          <circle cx="12" cy="12" r="10" />
          <path d="M12 8v5" />
          <path d="M12 16h.01" />
        </svg>
      </div>
      <div className="alert-banner-body">
        <div className="alert-banner-title">{title}</div>
        <div className="alert-banner-msg">{message}</div>
      </div>
      {onClose && (
        <button type="button" className="alert-banner-close" onClick={onClose} aria-label="关闭">
          ×
        </button>
      )}
    </div>
  );
}
