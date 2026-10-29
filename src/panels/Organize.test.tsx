import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import Organize from "./Organize";

function mockBackend(pages: number, status: unknown = null) {
  invoke.mockImplementation(async (cmd: string) => {
    if (cmd === "pages_info") return { page_count: pages };
    if (cmd === "pages_thumbnail") return new ArrayBuffer(4);
    if (cmd === "pages_organize") return { job_id: "7" };
    if (cmd === "job_status") return status;
    throw new Error(`unexpected ${cmd}`);
  });
}

const done = { id: "7", state: { kind: "done" }, progress: 1, outputs: ["/a/doc-organize.pdf"] };

beforeEach(() => {
  invoke.mockReset();
});

describe("Organize", () => {
  it("renders one thumbnail per page", async () => {
    mockBackend(3);
    render(<Organize path="/a/doc.pdf" />);
    expect(await screen.findAllByRole("img")).toHaveLength(3);
    expect(invoke).toHaveBeenCalledWith("pages_thumbnail", { path: "/a/doc.pdf", page: 2, max_px: 160 });
  });

  it("requests thumbnails only for cards scrolled into view", async () => {
    mockBackend(3);
    let reveal: (el: Element) => void = () => {};
    const seen: Element[] = [];
    vi.stubGlobal("IntersectionObserver", class {
      constructor(cb: IntersectionObserverCallback) {
        reveal = (el) => cb([{ isIntersecting: true, target: el } as unknown as IntersectionObserverEntry], this as unknown as IntersectionObserver);
      }
      observe(el: Element) { seen.push(el); }
      unobserve() {}
      disconnect() {}
    });
    try {
      render(<Organize path="/a/doc.pdf" />);
      await screen.findAllByRole("listitem");
      const thumbCalls = () => invoke.mock.calls.filter((c) => c[0] === "pages_thumbnail");
      await act(async () => reveal(seen[1]));
      expect(await screen.findAllByRole("img")).toHaveLength(1);
      expect(thumbCalls()).toEqual([["pages_thumbnail", { path: "/a/doc.pdf", page: 1, max_px: 160 }]]);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("disables Apply with zero pages kept", async () => {
    mockBackend(1);
    render(<Organize path="/a/doc.pdf" />);
    const apply = await screen.findByRole("button", { name: "Apply" });
    await screen.findAllByRole("img");
    expect((apply as HTMLButtonElement).disabled).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Select page 1" }));
    await userEvent.click(screen.getByRole("button", { name: "Delete selected" }));
    expect((apply as HTMLButtonElement).disabled).toBe(true);
  });

  it("sends rotate, delete and reorder as one PagePlan", async () => {
    mockBackend(4, done);
    render(<Organize path="/a/doc.pdf" />);
    await screen.findAllByRole("img");
    await userEvent.click(screen.getByRole("button", { name: "Select page 2" }));
    await userEvent.click(screen.getByRole("button", { name: "Delete selected" }));
    await userEvent.click(screen.getByRole("button", { name: "Select page 3" }));
    await userEvent.click(screen.getByRole("button", { name: "Rotate right" }));
    // items now [1,3,4]; drag last (4) onto first slot
    const cards = screen.getAllByRole("listitem");
    fireEvent.dragStart(cards[2]);
    fireEvent.dragOver(cards[0]);
    fireEvent.drop(cards[0]);
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("pages_organize", {
        input: { input: "/a/doc.pdf", plan: [{ page: 3, rotate: 0 }, { page: 0, rotate: 0 }, { page: 2, rotate: 90 }] },
      }),
    );
    expect(await screen.findByText("/a/doc-organize.pdf")).toBeTruthy();
  });

  it("shows the failure message", async () => {
    mockBackend(1, { id: "7", state: { kind: "failed", message: "encrypted input: a password is required" }, progress: 0, outputs: [] });
    render(<Organize path="/a/doc.pdf" />);
    await screen.findAllByRole("img");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect((await screen.findByRole("alert")).textContent).toContain("password is required");
  });

  it("creates no object URL for a thumbnail that resolves after unmount", async () => {
    let release: (b: ArrayBuffer) => void = () => {};
    invoke.mockImplementation(async (cmd: string) =>
      cmd === "pages_info" ? { page_count: 1 } : new Promise<ArrayBuffer>((r) => { release = r; }),
    );
    vi.mocked(URL.createObjectURL).mockClear();
    const { unmount } = render(<Organize path="/a/doc.pdf" />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("pages_thumbnail", expect.anything()));
    unmount();
    await act(async () => release(new ArrayBuffer(4)));
    expect(URL.createObjectURL).not.toHaveBeenCalled();
  });
});

describe("job polling", () => {
  it("stops polling when the panel unmounts", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      mockBackend(1, { id: "7", state: { kind: "running" }, progress: 0.2, outputs: [] });
      const { unmount } = render(<Organize path="/a/doc.pdf" />);
      await screen.findAllByRole("img");
      await userEvent.click(screen.getByRole("button", { name: "Apply" }));
      await vi.advanceTimersByTimeAsync(600);
      const polls = () => invoke.mock.calls.filter((c) => c[0] === "job_status").length;
      expect(polls()).toBeGreaterThan(1);
      unmount();
      const n = polls();
      await vi.advanceTimersByTimeAsync(2000);
      expect(polls()).toBe(n);
    } finally {
      vi.useRealTimers();
    }
  });

  it("asks for the password of an encrypted PDF and uses it for every call", async () => {
    invoke.mockImplementation(async (cmd: string, args: { password?: string }) => {
      if (cmd === "pages_info") {
        if (args.password === "s3cret") return { page_count: 2 };
        throw args.password ? "wrong password" : "encrypted input: a password is required";
      }
      if (cmd === "pages_thumbnail") return new ArrayBuffer(4);
      if (cmd === "pages_organize") return { job_id: "7" };
      return done;
    });
    render(<Organize path="/a/doc.pdf" />);
    await userEvent.type(await screen.findByLabelText("Password for doc.pdf", {}, { timeout: 5000 }), "nope");
    await userEvent.click(screen.getByRole("button", { name: "Unlock and Retry" }));
    expect(await screen.findByText("Wrong password, try again.", {}, { timeout: 5000 })).toBeTruthy();
    const infos = () => invoke.mock.calls.filter((c) => c[0] === "pages_info").length;
    const before = infos();
    await userEvent.type(screen.getByLabelText("Password for doc.pdf"), "nope");
    await userEvent.click(screen.getByRole("button", { name: "Unlock and Retry" }));
    await waitFor(() => expect(infos()).toBe(before + 1), { timeout: 5000 });
    await userEvent.type(await screen.findByLabelText("Password for doc.pdf", {}, { timeout: 5000 }), "s3cret");
    await userEvent.click(screen.getByRole("button", { name: "Unlock and Retry" }));
    expect(await screen.findAllByRole("img", {}, { timeout: 5000 })).toHaveLength(2);
    expect(screen.queryByLabelText("Password for doc.pdf")).toBeNull();
    expect(invoke).toHaveBeenCalledWith("pages_thumbnail", { path: "/a/doc.pdf", page: 1, max_px: 160, password: "s3cret" });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("pages_organize", {
      input: { input: "/a/doc.pdf", password: "s3cret", plan: [{ page: 0, rotate: 0 }, { page: 1, rotate: 0 }] },
    }), { timeout: 5000 });
  });
});
