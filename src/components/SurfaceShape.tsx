import { useLayoutEffect, useRef, useState, type CSSProperties } from "react";

export type SurfaceEdge = "top" | "bottom" | "left" | "right";

interface Props {
  edge: SurfaceEdge;
  // Radius of the concave flares where the surface meets the screen edge, 0 when it has none
  flare: number;
  // Radius of the corners that point away from the screen edge
  radius: number;
  // Radius of the corners on the screen-edge side when there is no flare (a floating surface)
  edgeRadius?: number;
}

const INSET = 0.5;

// The surface's whole silhouette, flares included, as one path. It is both the clip of the fill
// layer (glass, texture, wallpaper) and the line drawn around it, so the flares can never differ
// from the body. Coordinates are the surface's own box; (ox, oy) shifts them into the fill layer,
// which reaches `flare` px past the box on the flare sides.
function silhouette(edge: SurfaceEdge, w: number, h: number, f: number, r: number, er: number, i: number, ox: number, oy: number): { d: string; closed: boolean } {
  const X = (x: number) => x + ox;
  const Y = (y: number) => y + oy;
  const P = (x: number, y: number) => `${X(x)} ${Y(y)}`;
  const arc = (rad: number, sweep: number, x: number, y: number) => `A ${rad} ${rad} 0 0 ${sweep} ${P(x, y)}`;
  const clamp = (v: number, max: number) => Math.max(0, Math.min(v, max));

  if (edge === "top") {
    const rr = clamp(r, Math.min(w / 2, h));
    const a = Math.max(0, rr - i);
    if (f > 0) {
      return { closed: false, d: `M ${P(-f, 0)} ${arc(f, 1, i, f)} V ${Y(h - rr)} ${arc(a, 0, rr, h - i)} H ${X(w - rr)} ${arc(a, 0, w - i, h - rr)} V ${Y(f)} ${arc(f, 1, w + f, 0)}` };
    }
    const te = clamp(er, Math.min(w / 2, h));
    const ta = Math.max(0, te - i);
    return { closed: true, d: `M ${P(i, h - rr)} V ${Y(te)} ${arc(ta, 1, te, i)} H ${X(w - te)} ${arc(ta, 1, w - i, te)} V ${Y(h - rr)} ${arc(a, 1, w - rr, h - i)} H ${X(rr)} ${arc(a, 1, i, h - rr)} Z` };
  }

  if (edge === "bottom") {
    const rr = clamp(r, Math.min(w / 2, h));
    const a = Math.max(0, rr - i);
    if (f > 0) {
      return { closed: false, d: `M ${P(-f, h)} ${arc(f, 0, i, h - f)} V ${Y(rr)} ${arc(a, 1, rr, i)} H ${X(w - rr)} ${arc(a, 1, w - i, rr)} V ${Y(h - f)} ${arc(f, 0, w + f, h)}` };
    }
    const be = clamp(er, Math.min(w / 2, h));
    if (be > 0) {
      const ba = Math.max(0, be - i);
      return { closed: true, d: `M ${P(i, h - be)} V ${Y(rr)} ${arc(a, 1, rr, i)} H ${X(w - rr)} ${arc(a, 1, w - i, rr)} V ${Y(h - be)} ${arc(ba, 1, w - be, h - i)} H ${X(be)} ${arc(ba, 1, i, h - be)} Z` };
    }
    return { closed: false, d: `M ${P(i, h)} V ${Y(rr)} ${arc(a, 1, rr, i)} H ${X(w - rr)} ${arc(a, 1, w - i, rr)} V ${Y(h)}` };
  }

  const rr = clamp(r, Math.min(w, h / 2));
  const a = Math.max(0, rr - i);
  if (edge === "left") {
    return { closed: false, d: `M ${P(0, -f)} ${arc(f, 0, f, i)} H ${X(w - rr)} ${arc(a, 1, w - i, rr)} V ${Y(h - rr)} ${arc(a, 1, w - rr, h - i)} H ${X(f)} ${arc(f, 0, 0, h + f)}` };
  }
  return { closed: false, d: `M ${P(w, -f)} ${arc(f, 1, w - f, i)} H ${X(rr)} ${arc(a, 0, i, rr)} V ${Y(h - rr)} ${arc(a, 0, rr, h - i)} H ${X(w - f)} ${arc(f, 1, w, h + f)}` };
}

export function SurfaceShape({ edge, flare, radius, edgeRadius = 0 }: Props) {
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
  const horizontal = edge === "top" || edge === "bottom";
  const ox = horizontal ? flare : 0;
  const oy = horizontal ? 0 : flare;
  const ready = w > 0 && h > 0;

  const fill = ready ? silhouette(edge, w, h, flare, radius, edgeRadius, 0, ox, oy) : null;
  const line = ready ? silhouette(edge, w, h, flare, radius, edgeRadius, INSET, 0, 0) : null;

  const fillStyle: CSSProperties = {
    left: horizontal ? -flare : 0,
    right: horizontal ? -flare : 0,
    top: horizontal ? 0 : -flare,
    bottom: horizontal ? 0 : -flare,
    clipPath: fill ? `path('${fill.d}${fill.closed ? "" : " Z"}')` : "inset(0 0 100% 0)",
  };

  return (
    <>
      <div className="surface-fill" style={fillStyle} aria-hidden="true" />
      <svg ref={ref} className="edge-outline" aria-hidden>
        {line && <path d={line.d} />}
      </svg>
    </>
  );
}
