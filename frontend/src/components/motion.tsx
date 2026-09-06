import {
  CSSProperties,
  MouseEvent,
  ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";

type PageMotionProps = {
  children: ReactNode;
  className?: string;
};

/**
 * Page enter animation wrapper (CSS-driven).
 */
export function PageMotion({ children, className = "page page-motion" }: PageMotionProps) {
  return <div className={className}>{children}</div>;
}

type StaggerProps = {
  children: ReactNode;
  className?: string;
};

/** Children animate in sequence via CSS nth-child delays. */
export function Stagger({ children, className = "" }: StaggerProps) {
  return <div className={`stagger ${className}`.trim()}>{children}</div>;
}

export function StaggerItem({
  children,
  className = "",
  style,
}: {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
}) {
  return (
    <div className={`stagger-item ${className}`.trim()} style={style}>
      {children}
    </div>
  );
}

type GlassCardProps = {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  spotlight?: boolean;
};

/**
 * Glass panel with mouse-tracking spotlight.
 */
export function GlassCard({ children, className = "", style, spotlight = true }: GlassCardProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [spot, setSpot] = useState({ x: 50, y: 40 });

  const onMove = (e: MouseEvent) => {
    if (!spotlight || !ref.current) return;
    const r = ref.current.getBoundingClientRect();
    setSpot({
      x: ((e.clientX - r.left) / r.width) * 100,
      y: ((e.clientY - r.top) / r.height) * 100,
    });
  };

  return (
    <div
      ref={ref}
      className={`glass-card ${className}`.trim()}
      style={style}
      onMouseMove={onMove}
    >
      {spotlight && (
        <div
          className="glass-spotlight"
          style={{
            background: `radial-gradient(480px circle at ${spot.x}% ${spot.y}%, rgba(45,212,191,0.14), transparent 45%)`,
          }}
        />
      )}
      <div className="glass-card-inner">{children}</div>
    </div>
  );
}

type CountUpProps = {
  value: number;
  className?: string;
};

/** Animated number counter for dashboard stats. */
export function CountUp({ value, className }: CountUpProps) {
  const [display, setDisplay] = useState(0);
  const fromRef = useRef(0);

  useEffect(() => {
    const from = fromRef.current;
    const to = value;
    const start = performance.now();
    const dur = 900;
    let raf = 0;

    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / dur);
      const eased = 1 - Math.pow(1 - t, 3);
      setDisplay(Math.round(from + (to - from) * eased));
      if (t < 1) raf = requestAnimationFrame(tick);
      else fromRef.current = to;
    };

    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [value]);

  return <span className={className}>{display.toLocaleString()}</span>;
}

type ShineButtonProps = {
  children: ReactNode;
  className?: string;
  type?: "button" | "submit" | "reset";
  disabled?: boolean;
  onClick?: () => void;
};

/** Primary CTA with shimmer sweep. */
export function ShineButton({
  children,
  className = "btn btn-primary shine-btn",
  type = "button",
  disabled,
  onClick,
}: ShineButtonProps) {
  return (
    <button type={type} className={className} disabled={disabled} onClick={onClick}>
      <span className="shine-btn-label">{children}</span>
      <span className="shine-btn-glare" aria-hidden />
    </button>
  );
}

/** Floating ambient orbs. */
export function AmbientOrbs() {
  return (
    <div className="ambient-orbs" aria-hidden>
      <div className="orb orb-a" />
      <div className="orb orb-b" />
      <div className="orb orb-c" />
    </div>
  );
}

/** Film grain overlay for depth. */
export function GrainOverlay() {
  return <div className="grain-overlay" aria-hidden />;
}
