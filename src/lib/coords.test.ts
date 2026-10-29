import { describe, expect, it } from "vitest";
import { displayedSize, fromPdfSpace, type PageBoxes, toPdfSpace } from "./coords";

// CropBox [50 20 250 120] (200 x 100 pt) inside MediaBox [0 0 300 200].
const page = (rotation: number): PageBoxes => ({ media_box: [0, 0, 300, 200], crop_box: [50, 20, 250, 120], rotation });
const canvasOf = (p: PageBoxes) => {
  const { width, height } = displayedSize(p);
  return { left: 10, top: 30, width: width * 2, height: height * 2 }; // drawn at 200%, offset
};

describe("coords", () => {
  it("displays the CropBox, swapped for 90 and 270", () => {
    expect(displayedSize(page(0))).toEqual({ width: 200, height: 100 });
    expect(displayedSize(page(90))).toEqual({ width: 100, height: 200 });
    expect(displayedSize(page(180))).toEqual({ width: 200, height: 100 });
    expect(displayedSize(page(270))).toEqual({ width: 100, height: 200 });
  });

  it("maps the top-left canvas point of a 90-degree page to the CropBox's bottom-left", () => {
    const p = page(90);
    expect(toPdfSpace({ x: 10, y: 30 }, p, canvasOf(p))).toEqual([50, 20]);
  });

  // Displayed top-left, top-right, bottom-left corners -> user space, per /Rotate.
  it.each([
    [0, [50, 120], [250, 120], [50, 20]],
    [90, [50, 20], [50, 120], [250, 20]],
    [180, [250, 20], [50, 20], [250, 120]],
    [270, [250, 120], [250, 20], [50, 120]],
  ])("maps the corners of a page rotated %i", (rot, tl, tr, bl) => {
    const p = page(rot);
    const c = canvasOf(p);
    expect(toPdfSpace({ x: c.left, y: c.top }, p, c)).toEqual(tl);
    expect(toPdfSpace({ x: c.left + c.width, y: c.top }, p, c)).toEqual(tr);
    expect(toPdfSpace({ x: c.left, y: c.top + c.height }, p, c)).toEqual(bl);
  });

  it.each([0, 90, 180, 270])("round-trips an inner point at rotation %i", (rot) => {
    const p = page(rot);
    const c = canvasOf(p);
    const pt = { x: c.left + c.width * 0.3, y: c.top + c.height * 0.7 };
    const back = fromPdfSpace(toPdfSpace(pt, p, c), p, c);
    expect(back.x).toBeCloseTo(pt.x);
    expect(back.y).toBeCloseTo(pt.y);
  });

  it("keeps a point dragged off the page inside the CropBox", () => {
    const p = page(0);
    const c = canvasOf(p);
    expect(toPdfSpace({ x: -500, y: 5000 }, p, c)).toEqual([50, 20]);
    expect(toPdfSpace({ x: 5000, y: -500 }, p, c)).toEqual([250, 120]);
  });
});
