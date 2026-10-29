import { describe, expect, it } from "vitest";
import { bounds, kindOf, makeMark, type Style } from "./annots";

const rect = { left: 10, bottom: 20, right: 110, top: 70 };
const style: Style = { color: "#ff0000", width: 2, text: "Hi", fontSize: 12 };

describe("makeMark", () => {
  it("builds each rect tool's annotation as the core expects it", () => {
    expect(makeMark("highlight", { rect }, style)).toEqual({ type: "highlight", rect, color: "#ff0000" });
    expect(makeMark("strikeout", { rect }, style)).toEqual({ type: "strikeout", rect, color: "#ff0000" });
    expect(makeMark("note", { rect }, style)).toEqual({ type: "note", rect, text: "Hi" });
    expect(makeMark("free_text", { rect }, style)).toEqual({ type: "free_text", rect, text: "Hi", font_size: 12, color: "#ff0000" });
    expect(makeMark("ellipse", { rect }, style)).toEqual({ type: "shape", kind: "ellipse", rect, color: "#ff0000", width: 2 });
    expect(makeMark("rectangle", { rect }, style)).toEqual({ type: "shape", kind: "rectangle", rect, color: "#ff0000", width: 2 });
    expect(makeMark("redact", { rect }, style)).toEqual({ type: "redact", rect });
  });

  it("builds one ink stroke from the dragged points", () => {
    const points = [{ x: 1, y: 2 }, { x: 3, y: 4 }];
    expect(makeMark("ink", { points }, style)).toEqual({ type: "ink", strokes: [points], width: 2, color: "#ff0000" });
  });

  it("says why it cannot: no text for a note, a dot for ink", () => {
    expect(makeMark("note", { rect }, { ...style, text: "  " })).toBe("Type the text first.");
    expect(makeMark("free_text", { rect }, { ...style, text: "" })).toBe("Type the text first.");
    expect(makeMark("ink", { points: [{ x: 1, y: 2 }] }, style)).toBe("Draw a longer line.");
  });
});

describe("bounds", () => {
  it("is the rect, or the box around an ink stroke", () => {
    expect(bounds({ type: "redact", rect })).toEqual(rect);
    const ink = makeMark("ink", { points: [{ x: 5, y: 9 }, { x: 1, y: 30 }, { x: 8, y: 2 }] }, style);
    expect(typeof ink === "object" && bounds(ink)).toEqual({ left: 1, bottom: 2, right: 8, top: 30 });
  });

  it("names shapes by their kind", () => {
    expect(kindOf({ type: "shape", kind: "ellipse", rect, color: "#000000", width: 1 })).toBe("ellipse");
    expect(kindOf({ type: "redact", rect })).toBe("redact");
  });
});
