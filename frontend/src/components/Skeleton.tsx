type Props = {
  /** Number of skeleton cards in a row */
  count?: number;
  className?: string;
};

/**
 * Loading placeholders inspired by Ant Design Statistic loading + shadcn Skeleton.
 */
export function StatSkeleton({ count = 4 }: Props) {
  return (
    <div className="stats-row" aria-busy="true" aria-label="加载中">
      {Array.from({ length: count }).map((_, i) => (
        <div className="stat-card skeleton-card" key={i}>
          <div className="skeleton-line" style={{ width: "42%" }} />
          <div className="skeleton-line skeleton-lg" style={{ width: "58%" }} />
        </div>
      ))}
    </div>
  );
}

/** Inline pulse bar for list rows */
export function LineSkeleton({ width = "70%" }: { width?: string | number }) {
  return <div className="skeleton-line" style={{ width }} />;
}
