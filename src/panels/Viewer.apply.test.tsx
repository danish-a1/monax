import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import Viewer from "./Viewer";
import { ENCRYPTED } from "../lib/invoke";
import { backend, drag, INFO, sent } from "./viewerTestKit";

const click = (name: string) => userEvent.click(screen.getByRole("button", { name }));
const marks = () => screen.getByRole("list", { name: "Pending Marks" }).querySelectorAll("li").length;

beforeEach(() => { invoke.mockReset(); });

describe("Viewer applying marks", () => {
  it("asks for a password, then opens the page and runs both jobs with it", async () => {
    backend(invoke, ({ password }) => { if (password !== "pw") throw ENCRYPTED; return INFO; });
    render(<Viewer path="/a/Lock.pdf" />);
    await userEvent.type(await screen.findByLabelText("Password for Lock.pdf"), "pw");
    await click("Unlock and Retry");
    expect(await screen.findByRole("img", { name: "Page 1" })).toBeTruthy();
    expect(sent(invoke, "pages_thumbnail")[0]).toMatchObject({ path: "/a/Lock.pdf", password: "pw" });
    await click("Highlight");
    drag(0, 0, 100, 100);
    await click("Redaction Box");
    drag(0, 0, 100, 100);
    await click("Save Annotations");
    await screen.findByText("Annotations saved to /a/Doc-out.pdf");
    await click("Apply Redactions");
    await click("Redact Now");
    await screen.findByText("Redacted copy saved to /a/Doc-out.pdf (this view still shows the original)");
    expect(sent(invoke, "annotate_save")[0]).toMatchObject({ input: { input: "/a/Lock.pdf", password: "pw" } });
    expect(sent(invoke, "security_redact")[0]).toMatchObject({ input: { input: "/a/Lock.pdf", password: "pw" } });
  });

  it("drops only the marks a finished job applied and names its output", async () => {
    backend(invoke);
    render(<Viewer path="/a/Doc.pdf" />);
    await screen.findByRole("img", { name: "Page 1" });
    await click("Highlight");
    drag(0, 0, 100, 100);
    await click("Redaction Box");
    drag(0, 0, 100, 100);
    expect(marks()).toBe(2);
    await click("Save Annotations");
    expect(await screen.findByText("Annotations saved to /a/Doc-out.pdf")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Redact on page 1" })).toBeTruthy();
    expect(marks()).toBe(1);
    await click("Apply Redactions");
    await click("Redact Now");
    expect(await screen.findByText("Redacted copy saved to /a/Doc-out.pdf (this view still shows the original)")).toBeTruthy();
    expect(marks()).toBe(0);
  });

  it("keeps the marks when the job fails", async () => {
    backend(invoke);
    const answer = invoke.getMockImplementation()!;
    invoke.mockImplementation(async (cmd: string, a: object) =>
      cmd === "job_status" ? { id: "5", state: { kind: "failed", message: "damaged file" }, progress: 0, outputs: [] } : answer(cmd, a));
    render(<Viewer path="/a/Doc.pdf" />);
    await screen.findByRole("img", { name: "Page 1" });
    await click("Highlight");
    drag(0, 0, 100, 100);
    await click("Save Annotations");
    expect(await screen.findByText("damaged file")).toBeTruthy();
    expect(marks()).toBe(1);
    expect(screen.queryByText(/saved to/)).toBeNull();
  });

  it("clears a render failure once another page renders", async () => {
    backend(invoke, undefined, ({ page }) => { if (page === 0) throw "damaged file"; return new ArrayBuffer(4); });
    render(<Viewer path="/a/Doc.pdf" />);
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "damaged file");
    await click("Next Page");
    await screen.findByRole("img", { name: "Page 2" });
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
