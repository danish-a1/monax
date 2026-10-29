import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import ToolPanel from "./ToolPanel";
import { TOOLS } from "../tools";
import { ENCRYPTED } from "../lib/invoke";

const tool = (id: string) => TOOLS.find((t) => t.id === id)!;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string) =>
    cmd === "job_status"
      ? { id: "9", state: { kind: "done" }, progress: 1, outputs: ["/a/x-rotate.pdf"] }
      : { job_id: "9" },
  );
});

describe("ToolPanel passwords", () => {
  it("asks for the password of an encrypted input, retries with it, then clears it", async () => {
    let tries = 0;
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd !== "job_status") return { job_id: String(++tries) };
      const msg = tries === 1 ? ENCRYPTED : tries === 2 ? "wrong password" : null;
      return msg
        ? { id: String(tries), state: { kind: "failed", message: msg }, progress: 0, outputs: [] }
        : { id: "3", state: { kind: "done" }, progress: 1, outputs: ["/a/x-compress.pdf"] };
    });
    render(<ToolPanel tool={tool("compress")} files={["/a/x.pdf"]} />);
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    const pw = (await screen.findByLabelText("Password for x.pdf", {}, { timeout: 5000 })) as HTMLInputElement;
    expect([pw.type, pw.getAttribute("autocomplete")]).toEqual(["password", "off"]);
    await userEvent.type(pw, "nope");
    await userEvent.click(screen.getByRole("button", { name: "Unlock and Retry" }));
    expect(await screen.findByText("Wrong password, try again.", {}, { timeout: 5000 })).toBeTruthy();
    const again = screen.getByLabelText("Password for x.pdf") as HTMLInputElement;
    expect(again.value).toBe("");
    await userEvent.type(again, "s3cret");
    await userEvent.click(screen.getByRole("button", { name: "Unlock and Retry" }));
    expect(await screen.findByText("/a/x-compress.pdf", {}, { timeout: 5000 })).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("content_compress", {
      input: { input: "/a/x.pdf", password: "s3cret", opts: { discard_media: false, strip_metadata: false } },
    });
    expect(screen.queryByLabelText("Password for x.pdf")).toBeNull();
  });

  it("asks one password per file for merge", async () => {
    invoke.mockImplementation(async (cmd: string) =>
      cmd === "job_status" ? { id: "1", state: { kind: "failed", message: ENCRYPTED }, progress: 0, outputs: [] } : { job_id: "1" });
    render(<ToolPanel tool={tool("merge")} files={["/a/x.pdf", "/a/y.pdf"]} />);
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await userEvent.type(await screen.findByLabelText("Password for y.pdf", {}, { timeout: 5000 }), "py");
    expect(screen.getByLabelText("Password for x.pdf")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Unlock and Retry" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("pages_merge", {
      input: { inputs: ["/a/x.pdf", "/a/y.pdf"], opts: { blank_before_even: false }, passwords: [null, "py"] },
    }), { timeout: 5000 });
  });

  it("retries encrypt on a protected input with its option passwords and the input password", async () => {
    let tries = 0;
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd !== "job_status") return { job_id: String(++tries) };
      return tries === 1
        ? { id: "1", state: { kind: "failed", message: ENCRYPTED }, progress: 0, outputs: [] }
        : { id: "2", state: { kind: "done" }, progress: 1, outputs: ["/a/x-encrypt.pdf"] };
    });
    render(<ToolPanel tool={tool("encrypt")} files={["/a/x.pdf"]} />);
    await userEvent.type(screen.getByLabelText("User Password"), "u");
    await userEvent.type(screen.getByLabelText("Owner Password"), "o");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await userEvent.type(await screen.findByLabelText("Password for x.pdf", {}, { timeout: 5000 }), "in");
    await userEvent.click(screen.getByRole("button", { name: "Unlock and Retry" }));
    expect(await screen.findByText("/a/x-encrypt.pdf", {}, { timeout: 5000 })).toBeTruthy();
    const sent = invoke.mock.calls.filter((c) => c[0] === "security_encrypt").map((c) => c[1].input);
    expect(sent[1]).toMatchObject({ input: "/a/x.pdf", password: "in", opts: { user_pw: "u", owner_pw: "o" } });
    expect((screen.getByLabelText("Owner Password") as HTMLInputElement).value).toBe("");
  });

  it("does not prompt for tools that take a password themselves", async () => {
    invoke.mockImplementation(async (cmd: string) =>
      cmd === "job_status" ? { id: "1", state: { kind: "failed", message: "wrong password" }, progress: 0, outputs: [] } : { job_id: "1" });
    render(<ToolPanel tool={tool("decrypt")} files={["/a/x.pdf"]} />);
    await userEvent.type(screen.getByLabelText("Password"), "p");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(await screen.findByRole("alert", {}, { timeout: 5000 })).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("security_decrypt", { input: { input: "/a/x.pdf", password: "p" } });
    expect((screen.getByLabelText("Password") as HTMLInputElement).value).toBe("");
    expect(screen.queryByRole("button", { name: "Unlock and Retry" })).toBeNull();
  });
});
