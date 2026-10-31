import { useLayoutEffect, useRef, useState } from "react";

interface Props {
  progress: number;
  topRadius: number;
  flare: number;
  bottomRadius: number;
  paused: boolean;
  finished: boolean;
  title?: string;
}

const STROKE = 2;

export function NotchProgressBorder({ progress, topRadius, flare, bottomRadius, paused, finished, title }: Props) {
  const ref = useRef<SVGSVGElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setSize((prev) => (prev.w === el.clientWidth && prev.h === el.clientHeight ? prev : { w: el.clientWidth, h: el.clientHeight }));
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const { w, h } = size;
  const inset = STROKE / 2;
  const scale = w > 0 ? Math.min(1, w / (2 * bottomRadius), h / (topRadius + bottomRadius || 1)) : 1;
  const rb = bottomRadius * scale;
  const rt = topRadius * scale;
  const arc = Math.max(0, rb - inset);
  const sides = flare > 0 ? flare : rt;
  const start = flare > 0 ? `M ${-flare} 0 A ${flare} ${flare} 0 0 1 ${inset} ${flare}` : `M ${inset} ${rt}`;
  const end = flare > 0 ? ` A ${flare} ${flare} 0 0 1 ${w + flare} 0` : "";
  const path = w > 0
    ? `${start} V ${h - rb} A ${arc} ${arc} 0 0 0 ${rb} ${h - inset} H ${w - rb} A ${arc} ${arc} 0 0 0 ${w - inset} ${h - rb} V ${sides}${end}`
    : "";

  return (
    <svg ref={ref} className={`notch-progress${paused ? " paused" : ""}${finished ? " finished" : ""}`} aria-hidden>
      {title && <title>{title}</title>}
      {path && (
        <>
          <path d={path} className="notch-progress-track" />
          <path d={path} className="notch-progress-fill" pathLength={100} strokeDasharray={`${Math.max(0, Math.min(100, progress))} 100`} />
        </>
      )}
    </svg>
  );
}
