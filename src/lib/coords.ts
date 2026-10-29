/** `[left, bottom, right, top]` in PDF user space points. */
export type Box = [number, number, number, number];
/** A page's boxes as `pages_info` reports them; `rotation` is /Rotate, clockwise degrees. */
export interface PageBoxes { media_box: Box; crop_box: Box; rotation: number }
export interface Point { x: number; y: number }
/** Where the page is drawn: viewport (or element-local) pixels, top-left origin. */
export interface CanvasRect { left: number; top: number; width: number; height: number }
/** PDF user space rect, points, y-up (the core's `Rect`). */
export interface Rect { left: number; bottom: number; right: number; top: number }

/** Displayed size in points: the CropBox, swapped for /Rotate 90 or 270. */
export function displayedSize(page: PageBoxes): { width: number; height: number } {
  const [l, b, r, t] = page.crop_box;
  return page.rotation % 180 ? { width: t - b, height: r - l } : { width: r - l, height: t - b };
}

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

/**
 * A point on the drawn page -> PDF user space `[x, y]` (points, y-up, unrotated MediaBox),
 * kept inside the CropBox. `canvas` is where the page as displayed (CropBox, /Rotate) is drawn.
 */
export function toPdfSpace(pt: Point, page: PageBoxes, canvas: CanvasRect): [number, number] {
  const [l, b, r, t] = page.crop_box;
  const [w, h] = [r - l, t - b];
  // u, v: 0..1 across and down the displayed page.
  const u = clamp((pt.x - canvas.left) / canvas.width, 0, 1);
  const v = clamp((pt.y - canvas.top) / canvas.height, 0, 1);
  const [x, y] =
    page.rotation === 90 ? [l + v * w, b + u * h]
      : page.rotation === 180 ? [r - u * w, b + v * h]
        : page.rotation === 270 ? [r - v * w, t - u * h]
          : [l + u * w, t - v * h];
  return [clamp(x, l, r), clamp(y, b, t)];
}

/** PDF user space `[x, y]` -> the point where it is drawn on `canvas` (inverse of `toPdfSpace`). */
export function fromPdfSpace([x, y]: [number, number], page: PageBoxes, canvas: CanvasRect): Point {
  const [l, b, r, t] = page.crop_box;
  const [w, h] = [r - l, t - b];
  const [u, v] =
    page.rotation === 90 ? [(y - b) / h, (x - l) / w]
      : page.rotation === 180 ? [(r - x) / w, (y - b) / h]
        : page.rotation === 270 ? [(t - y) / h, (r - x) / w]
          : [(x - l) / w, (t - y) / h];
  return { x: canvas.left + u * canvas.width, y: canvas.top + v * canvas.height };
}
