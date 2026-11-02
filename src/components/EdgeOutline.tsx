import { useLayoutEffect, useRef, useState } from "react";

export type OutlineEdge = "top" | "bottom" | "left" | "right";

interface Props {
  edge: OutlineEdge;
  // Radius of the concave flares where the surface meets the screen edge, 0 when it floats
  flare: number;
  // Radius of the corners that point away from the screen edge
  radius: number;
  // Radius of the corners on the screen-edge side when there is no flare (a floating surface)
  edgeRadius?: number;
}

const INSET = 0.5;

// One continuous line that follows the whole surface, flares included, and ends at the screen
// edge. A box-shadow rim can't do that: it runs along the straight sides and stops dead where the
// flare starts.
function outlinePath(edge: OutlineEdge, w: number, h: number, f: number, r: number, er: number): string {
  const i = INSET;
  if (edge === "top") {
    const rr = Math.min(r, w / 2, h);
    const a = Math.max(0, rr - i);
    if (f > 0) {
      return `M ${-f} 0 A ${f} ${f} 0 0 1 ${i} ${f} V ${h - rr} A ${a} ${a} 0 0 0 ${rr} ${h - i} H ${w - rr} A ${a} ${a} 0 0 0 ${w - i} ${h - rr} V ${f} A ${f} ${f} 0 0 1 ${w + f} 0`;
    }
    const te = Math.min(er, w / 2, h);
    const ta = Math.max(0, te - i);
    return `M ${i} ${h - rr} V ${te} A ${ta} ${ta} 0 0 1 ${te} ${i} H ${w - te} A ${ta} ${ta} 0 0 1 ${w - i} ${te} V ${h - rr} A ${a} ${a} 0 0 1 ${w - rr} ${h - i} H ${rr} A ${a} ${a} 0 0 1 ${i} ${h - rr} Z`;
  }
  if (edge === "bottom") {
    const rr = Math.min(r, w / 2, h);
    const a = Math.max(0, rr - i);
    if (f > 0) {
      return `M ${-f} ${h} A ${f} ${f} 0 0 0 ${i} ${h - f} V ${rr} A ${a} ${a} 0 0 1 ${rr} ${i} H ${w - rr} A ${a} ${a} 0 0 1 ${w - i} ${rr} V ${h - f} A ${f} ${f} 0 0 0 ${w + f} ${h}`;
    }
    return `M ${i} ${h} V ${rr} A ${a} ${a} 0 0 1 ${rr} ${i} H ${w - rr} A ${a} ${a} 0 0 1 ${w - i} ${rr} V ${h}`;
  }
  const rr = Math.min(r, w, h / 2);
  const a = Math.max(0, rr - i);
  if (edge === "left") {
    return `M 0 ${-f} A ${f} ${f} 0 0 0 ${f} ${i} H ${w - rr} A ${a} ${a} 0 0 1 ${w - i} ${rr} V ${h - rr} A ${a} ${a} 0 0 1 ${w - rr} ${h - i} H ${f} A ${f} ${f} 0 0 0 0 ${h + f}`;
  }
  return `M ${w} ${-f} A ${f} ${f} 0 0 1 ${w - f} ${i} H ${rr} A ${a} ${a} 0 0 0 ${i} ${rr} V ${h - rr} A ${a} ${a} 0 0 0 ${rr} ${h - i} H ${w - f} A ${f} ${f} 0 0 1 ${w} ${h + f}`;
}

export function EdgeOutline({ edge, flare, radius, edgeRadius = 0 }: Props) {
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
  return (
    <svg ref={ref} className="edge-outline" aria-hidden>
      {w > 0 && h > 0 && <path d={outlinePath(edge, w, h, flare, radius, edgeRadius)} />}
    </svg>
  );
}
