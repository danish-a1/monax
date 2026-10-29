import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Pending, Tool } from "../lib/annots";
import type { PageBoxes } from "../lib/coords";
import AnnotLayer from "./AnnotLayer";

// CropBox [50 20 250 120] shown at /Rotate 90: displayed 100 x 200 pt, drawn at 200 x 400 px.
const page: PageBoxes = { media_box: [0, 0, 300, 200], crop_box: [50, 20, 250, 120], rotation: 90 };
const rect = { left: 100, bottom: 40, right: 150, top: 70 };

function setup(tool: Tool, items: Pending[] = [], selected: number | null = null) {
  const onDraw = vi.fn();
  const onSelect = vi.fn();
  render(<AnnotLayer page={page} width={200} height={400} tool={tool} items={items} selected={selected} onDraw={onDraw} onSelect={onSelect} />);
  // jsdom lays nothing out: the layer sits at the viewport origin, so client = local pixels.
  return { svg: screen.getByRole("group", { name: "Page annotations" }), onDraw, onSelect };
}
const drag = (el: Element, pts: [number, number][]) => {
  fireEvent.pointerDown(el, { clientX: pts[0][0], clientY: pts[0][1], pointerId: 1 });
  for (const [x, y] of pts.slice(1)) fireEvent.pointerMove(el, { clientX: x, clientY: y, pointerId: 1 });
  const [x, y] = pts[pts.length - 1];
  fireEvent.pointerUp(el, { clientX: x, clientY: y, pointerId: 1 });
};

describe("AnnotLayer", () => {
  it("reports a dragged rect in PDF user space, rotation undone", () => {
    const { svg, onDraw } = setup("highlight");
    drag(svg, [[0, 0], [100, 200]]); // displayed top-left quarter
    expect(onDraw).toHaveBeenCalledWith({ rect: { left: 50, bottom: 20, right: 150, top: 70 } });
  });

  it("reports an ink stroke's points in PDF user space", () => {
    const { svg, onDraw } = setup("ink");
    drag(svg, [[0, 0], [200, 0], [200, 400]]);
    expect(onDraw).toHaveBeenCalledWith({ points: [{ x: 50, y: 20 }, { x: 50, y: 120 }, { x: 250, y: 120 }] });
  });

  it("ignores a click, a drag in select mode, and a drawing cancelled with Escape", () => {
    const { svg, onDraw } = setup("rectangle");
    drag(svg, [[10, 10], [11, 11]]);
    fireEvent.pointerDown(svg, { clientX: 0, clientY: 0, pointerId: 1 });
    fireEvent.pointerMove(svg, { clientX: 50, clientY: 50, pointerId: 1 });
    expect(svg.querySelector(".draft")).not.toBeNull();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(svg.querySelector(".draft")).toBeNull();
    fireEvent.pointerUp(svg, { clientX: 50, clientY: 50, pointerId: 1 });
    expect(onDraw).not.toHaveBeenCalled();
    cleanup();
    const sel = setup("select");
    drag(sel.svg, [[0, 0], [100, 100]]);
    expect(sel.onDraw).not.toHaveBeenCalled();
  });

  it("draws pending marks where they sit on the displayed page and selects one on click", () => {
    const items: Pending[] = [
      { id: 7, page: 0, mark: { type: "highlight", rect, color: "#ffff00" } },
      { id: 8, page: 0, mark: { type: "shape", kind: "ellipse", rect, color: "#ff0000", width: 2 } },
      { id: 9, page: 0, mark: { type: "ink", strokes: [[{ x: 50, y: 20 }, { x: 250, y: 120 }]], width: 2, color: "#000000" } },
      { id: 10, page: 0, mark: { type: "redact", rect } },
    ];
    const { onSelect } = setup("select", items, 8);
    const hl = screen.getByLabelText("Highlight 1").querySelector("rect")!;
    // x 100..150 -> down 100..200 px; y 40..70 -> across 40..100 px (90 deg).
    expect([hl.getAttribute("x"), hl.getAttribute("y"), hl.getAttribute("width"), hl.getAttribute("height")]).toEqual(["40", "100", "60", "100"]);
    expect(screen.getByLabelText("Ellipse 2").querySelector("ellipse")).not.toBeNull();
    expect(screen.getByLabelText("Ellipse 2").getAttribute("class")).toContain("selected");
    expect(screen.getByLabelText("Ink 3").querySelector("polyline")!.getAttribute("points")).toBe("0,0 200,400");
    expect(screen.getByLabelText("Redact 4").getAttribute("class")).toContain("redact");
    fireEvent.click(screen.getByLabelText("Highlight 1"));
    expect(onSelect).toHaveBeenCalledWith(7);
  });
});
