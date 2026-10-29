import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { pollJob } from "./invoke";
import { parsePages } from "../tools";

const running = { id: "1", state: { kind: "running" }, progress: 0.5, outputs: [] };
const done = { ...running, state: { kind: "done" }, progress: 1 };
const tick = (ms = 30) => vi.advanceTimersByTimeAsync(ms);

beforeEach(() => {
  invoke.mockReset();
  vi.useFakeTimers();
});
afterEach(() => vi.useRealTimers());

describe("pollJob", () => {
  it("polls until a terminal state", async () => {
    invoke.mockResolvedValueOnce(running).mockResolvedValue(done);
    const seen: unknown[] = [];
    pollJob("1", (j) => seen.push(j?.state.kind), 5);
    await tick();
    expect(seen).toEqual(["running", "done"]);
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("reports null once and stops", async () => {
    invoke.mockResolvedValue(null);
    const cb = vi.fn();
    pollJob("1", cb, 5);
    await tick();
    expect(cb).toHaveBeenCalledExactlyOnceWith(null);
  });

  it("surfaces an IPC error and stops", async () => {
    invoke.mockRejectedValue("backend down");
    const cb = vi.fn();
    pollJob("1", cb, 5);
    await tick();
    expect(cb).toHaveBeenCalledExactlyOnceWith(null, "backend down");
  });

  it("stop() ends polling", async () => {
    invoke.mockResolvedValue(running);
    const stop = pollJob("1", () => {}, 5);
    await tick(12);
    const n = invoke.mock.calls.length;
    expect(n).toBeGreaterThan(1);
    stop();
    await tick(30);
    expect(invoke.mock.calls.length).toBe(n);
  });
});

describe("parsePages", () => {
  it("parses ranges 1-based to 0-based", () => expect(parsePages("1-3, 5")).toEqual([0, 1, 2, 4]));
  it.each(["", "0", "3-1", "a", "1-"])("rejects %j", (t) => expect(() => parsePages(t)).toThrow());
});
