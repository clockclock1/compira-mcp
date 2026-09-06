import { ReactNode } from "react";
import { Link } from "react-router-dom";

type Props = {
  title: string;
  description?: string;
  actionLabel?: string;
  actionTo?: string;
  onAction?: () => void;
  icon?: ReactNode;
};

/**
 * Empty state inspired by Ant Design Empty (via Compira):
 * image/title/description + optional primary CTA.
 */
export default function EmptyState({
  title,
  description,
  actionLabel,
  actionTo,
  onAction,
  icon,
}: Props) {
  return (
    <div className="empty-state">
      <div className="empty-state-icon" aria-hidden>
        {icon || (
          <svg viewBox="0 0 24 24" width="28" height="28">
            <rect x="3" y="5" width="18" height="14" rx="2" />
            <path d="M3 9h18" />
            <path d="M8 14h8" />
          </svg>
        )}
      </div>
      <div className="empty-state-title">{title}</div>
      {description && <div className="empty-state-desc">{description}</div>}
      {actionLabel && actionTo && (
        <Link to={actionTo} className="btn btn-primary empty-state-cta">
          {actionLabel}
        </Link>
      )}
      {actionLabel && onAction && !actionTo && (
        <button type="button" className="btn btn-primary empty-state-cta" onClick={onAction}>
          {actionLabel}
        </button>
      )}
    </div>
  );
}
