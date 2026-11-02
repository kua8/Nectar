// Color grading from artwork, after Panic's iTunes 11 algorithm
// (https://blog.panic.com/itunes-11-and-colors/) and its Mathematica
// approximation: two most common perceptually-distinct colors (YUV distance)
// become card + accent, white/black text whichever reads. Plain math, no deps.

export interface ArtTheme {
  background: string; // CSS color, "" = keep the default neutral card
  text: string; // near-white or near-black, whichever reads on the card
  accent: string; // CSS color for progress fill / timestamps
}

export const FALLBACK_THEME: ArtTheme = {
  background: "",
  text: "",
  accent: "",
};

export type RGB = [number, number, number];

// WCAG 2.x relative luminance.
function luminance([r, g, b]: RGB): number {
  const f = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

function contrast(l1: number, l2: number): number {
  const [hi, lo] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (hi + 0.05) / (lo + 0.05);
}

// YUV converts better than RGB for perceptual "are these colors different".
function yuvDist(a: RGB, b: RGB): number {
  const [r1, g1, b1] = a.map((v) => v / 255);
  const [r2, g2, b2] = b.map((v) => v / 255);
  const dy = 0.299 * (r1 - r2) + 0.587 * (g1 - g2) + 0.114 * (b1 - b2);
  const du = -0.14713 * (r1 - r2) - 0.28886 * (g1 - g2) + 0.436 * (b1 - b2);
  const dv = 0.615 * (r1 - r2) - 0.51499 * (g1 - g2) - 0.10001 * (b1 - b2);
  return Math.hypot(dy, du, dv);
}

function mix(a: RGB, b: RGB, t: number): RGB {
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
}

const css = (c: RGB) => `rgb(${c.map(Math.round).join(" ")})`;

// 64x64: smaller grids average away thin features (title script text) — the
// small vivid regions good accents come from.
const SIZE = 64;
// ponytail: knobs — min YUV distance between the two picked colors; the
// source's 0.2 on a 0..1 scale, re-tuned by eye against a cover test set.
const DISTINCT = 0.25;
// A bucket with every channel below this is black as far as the card is
// concerned — near-black greys and shadows all land here.
const DARK_CHANNEL = 48;
// ...and this is how much of the cover has to be that black before we stop
// arguing: a black cover, rather than a colorful one sitting in shadow.
const DARK_SHARE = 0.7;

const isBlack = ([r, g, b]: RGB) => Math.max(r, g, b) < DARK_CHANNEL;

interface Bucket {
  color: RGB;
  n: number; // pixel count
}

// Downsample to 64x64 (off-thread via <img>.decode()) and tally pixels into
// 12-bit color buckets ((r>>4)<<8 | (g>>4)<<4 | b>>4), most frequent first.
async function extract(src: string): Promise<Bucket[]> {
  const img = new Image();
  img.src = src;
  await img.decode();
  const canvas = new OffscreenCanvas(SIZE, SIZE);
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("no 2d context");
  ctx.drawImage(img, 0, 0, SIZE, SIZE);
  const d = ctx.getImageData(0, 0, SIZE, SIZE).data;

  const tallies = new Map<number, { n: number; r: number; g: number; b: number }>();
  for (let i = 0; i < d.length; i += 4) {
    if (d[i + 3] < 128) continue;
    const key = ((d[i] >> 4) << 8) | ((d[i + 1] >> 4) << 4) | (d[i + 2] >> 4);
    const t = tallies.get(key) ?? { n: 0, r: 0, g: 0, b: 0 };
    t.n++;
    t.r += d[i];
    t.g += d[i + 1];
    t.b += d[i + 2];
    tallies.set(key, t);
  }
  return [...tallies.values()]
    .sort((a, b) => b.n - a.n)
    .map((t) => ({ color: [t.r / t.n, t.g / t.n, t.b / t.n] as RGB, n: t.n }));
}

const WHITE: RGB = [250, 250, 250];
const BLACK: RGB = [18, 18, 18];
const WHITE_L = luminance(WHITE);
const BLACK_L = luminance(BLACK);
// Colorfulness (channel spread) — tiebreaker between equally-readable accents.
const chroma = ([r, g, b]: RGB) => Math.max(r, g, b) - Math.min(r, g, b);
// How much of the cover color survives into the card — the rest is the dark
// base, keeping the widget dark-anchored on every cover (alpha comes back
// below, but a 0.85 veil alone can't darken a white cover enough).
const NEUTRAL: RGB = [23, 23, 23]; // matches bg-neutral-900
const BG_STRENGTH = 0.75;

// Shared finishing: card color at BG_STRENGTH over the dark base, nudged
// until white or black text passes 4.5:1; first accent candidate that needs
// the fewest nudges to pass wins (ties → the more colorful). Used by both the
// extraction path and hand-authored colorways.
function finish(bgSource: RGB, accentCandidates: (RGB | undefined)[]): ArtTheme {
  // Card: nudged until one of white/black text passes 4.5:1 (an unreadable
  // mid-tone card slides toward whichever extreme text survives on).
  let bg = mix(NEUTRAL, bgSource, BG_STRENGTH);
  const towardWhite = contrast(luminance(bg), WHITE_L) >= contrast(luminance(bg), BLACK_L);
  const textColor = towardWhite ? WHITE : BLACK;
  for (let i = 0; i < 40 && contrast(luminance(bg), towardWhite ? WHITE_L : BLACK_L) < 4.5; i++) {
    bg = mix(bg, towardWhite ? BLACK : WHITE, 0.05);
  }
  const bgL = luminance(bg);

  const nudge = (c: RGB) => {
    let v = c;
    let i = 0;
    while (i < 12 && contrast(luminance(v), bgL) < 4.5) {
      v = mix(v, textColor, 0.15);
      i++;
    }
    return contrast(luminance(v), bgL) >= 4.5 ? { v, i } : null;
  };
  const candidates = accentCandidates
    .filter((c) => c !== undefined)
    .map((c) => ({ base: c, ...nudge(c) }))
    .filter((s) => s.v !== undefined && s.v !== null)
    .sort((a, b) => (a.i ?? 12) - (b.i ?? 12) || chroma(b.base) - chroma(a.base));
  let accent = candidates[0]?.v;
  if (!accent) accent = textColor;

  return {
    // 0.85 alpha restores the frosted card look — slash syntax required.
    background: `rgb(${bg.map(Math.round).join(" ")} / 0.85)`,
    text: css(textColor),
    accent: css(accent),
  };
}

// Hand-authored colorway (colorways.json): the submitted colors go through
// the same finishing pipeline, so community entries can't ship unreadable.
export function themeFromColors(bg: RGB, accent: RGB): ArtTheme {
  return finish(bg, [accent]);
}

export async function artTheme(art: string): Promise<ArtTheme> {
  let buckets: Bucket[];
  try {
    buckets = await extract(art);
  } catch {
    return FALLBACK_THEME;
  }
  if (buckets.length === 0) return FALLBACK_THEME;

  // Three most common perceptually-distinct colors: card, then two accent
  // candidates to choose from. Buckets closer than DISTINCT to an already-
  // picked color are skipped.
  const distinct: Bucket[] = [];
  for (const b of buckets) {
    if (distinct.every((d) => yuvDist(d.color, b.color) >= DISTINCT)) {
      distinct.push(b);
      if (distinct.length === 3) break;
    }
  }

  // Dark covers put black in the top bucket, and black as the card source
  // flattens the card to the neutral — the cover's color is thrown away.
  // When black really is most of the cover (DARK_SHARE) that neutral is the
  // honest answer; when it isn't, black is only the shadow the color lives
  // in, so the most common color that isn't black gets promoted instead.
  // That one comes from the raw bucket list rather than the distinct three:
  // DISTINCT also measures distance *from black*, so the case being fixed
  // here — a dark navy holding 30% of the cover — is exactly what it drops.
  const pixels = buckets.reduce((n, b) => n + b.n, 0);
  const blacks = buckets.reduce((n, b) => n + (isBlack(b.color) ? b.n : 0), 0);
  const color = buckets.find((b) => !isBlack(b.color));
  const bg = blacks < pixels * DARK_SHARE && color ? color : distinct[0];

  return finish(
    bg.color,
    distinct
      .slice(1)
      .filter((b) => b !== bg)
      .map((b) => b.color),
  );
}
