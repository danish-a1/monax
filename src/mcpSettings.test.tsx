import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import McpSettings from "./mcpSettings";

const token = "ab".repeat(32);
const url = "http://127.0.0.1:4242/mcp";
const snippet = `claude mcp add --transport http monax ${url} --header "Authorization: Bearer ${token}"`;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({ status: "running", url, token, port_changed: false });
});

describe("McpSettings", () => {
  it("shows the url and a masked token until revealed", async () => {
    render(<McpSettings />);
    expect(await screen.findByText(url)).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("mcp_info");
    expect(screen.queryByText(token)).toBeNull();
    expect(document.body.textContent).not.toContain(token);
    await userEvent.click(screen.getByRole("button", { name: "Reveal" }));
    expect(screen.getByText(token)).toBeTruthy();
    expect(screen.getByText(snippet)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Hide" }));
    expect(screen.queryByText(token)).toBeNull();
  });

  it("copies the token and the claude snippet", async () => {
    const writeText = vi.fn(async () => {});
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<McpSettings />);
    await userEvent.click(await screen.findByRole("button", { name: "Copy Token" }));
    expect(writeText).toHaveBeenLastCalledWith(token);
    expect(await screen.findByText("Copied")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Copy Command" }));
    expect(writeText).toHaveBeenLastCalledWith(snippet);
  });

  it("reveals the text for manual selection when the clipboard is blocked", async () => {
    const writeText = vi.fn(async () => Promise.reject(new Error("denied")));
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<McpSettings />);
    await userEvent.click(await screen.findByRole("button", { name: "Copy Command" }));
    expect(await screen.findByText("Clipboard unavailable: select the text and copy it.")).toBeTruthy();
    expect(screen.getByText(snippet)).toBeTruthy();
  });

  it("warns only when the port moved and the token was rotated", async () => {
    const warning = "Port changed - re-run the connection command.";
    const { unmount } = render(<McpSettings />);
    expect(await screen.findByText(url)).toBeTruthy();
    expect(screen.queryByText(warning)).toBeNull();
    unmount();
    invoke.mockResolvedValue({ status: "running", url, token, port_changed: true });
    render(<McpSettings />);
    expect(await screen.findByText(warning)).toBeTruthy();
  });

  it("shows the failure reason", async () => {
    invoke.mockResolvedValue({ status: "failed", reason: "Address already in use (/x)" });
    render(<McpSettings />);
    expect(await screen.findByText("MCP server failed to start: Address already in use (/x)")).toBeTruthy();
  });

  it("shows starting and command errors", async () => {
    invoke.mockResolvedValueOnce({ status: "starting" });
    const { unmount } = render(<McpSettings />);
    expect(await screen.findByText("MCP server is starting.")).toBeTruthy();
    unmount();
    invoke.mockRejectedValueOnce("boom");
    render(<McpSettings />);
    expect(await screen.findByText("boom")).toBeTruthy();
  });
});
