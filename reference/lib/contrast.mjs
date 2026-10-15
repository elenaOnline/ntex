// WCAG 2 contrast ratio and APCA Lc (APCA-W3 0.0.98G-4g constants), for the
// build report. Numbers are a check, not the authority: render and look too.

function hex(c) {
  const h = c.replace("#", "");
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16));
}

function wcagLum(c) {
  const [r, g, b] = hex(c).map((v) => {
    const s = v / 255;
    return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function wcag(fg, bg) {
  const a = wcagLum(fg), b = wcagLum(bg);
  const [hi, lo] = a > b ? [a, b] : [b, a];
  return (hi + 0.05) / (lo + 0.05);
}

function apcaY(c) {
  const [r, g, b] = hex(c).map((v) => Math.pow(v / 255, 2.4));
  return 0.2126729 * r + 0.7151522 * g + 0.072175 * b;
}

export function apca(fg, bg) {
  const blkThrs = 0.022, blkClmp = 1.414;
  let t = apcaY(fg), y = apcaY(bg);
  t = t > blkThrs ? t : t + Math.pow(blkThrs - t, blkClmp);
  y = y > blkThrs ? y : y + Math.pow(blkThrs - y, blkClmp);
  if (Math.abs(y - t) < 0.0005) return 0;
  let out;
  if (y > t) {
    const s = (Math.pow(y, 0.56) - Math.pow(t, 0.57)) * 1.14;
    out = s < 0.1 ? 0 : s - 0.027;
  } else {
    const s = (Math.pow(y, 0.65) - Math.pow(t, 0.62)) * 1.14;
    out = s > -0.1 ? 0 : s + 0.027;
  }
  return out * 100;
}

/** rows of { name, fg, bg, wcag, apca, need } */
export function contrastTable(pairs) {
  return pairs.map(([name, fg, bg, need]) => ({ name, fg, bg, wcag: wcag(fg, bg), apca: apca(fg, bg), need }));
}
