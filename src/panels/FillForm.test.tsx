import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import FillForm from "./FillForm";
import { ENCRYPTED } from "../lib/invoke";

const rect = { left: 0, bottom: 0, right: 10, top: 10 };
const field = (name: string, kind: string, value: string, extra = {}) =>
  ({ name, kind, page: 0, rect, value, read_only: false, ...extra });
const FIELDS = [
  field("person.name", "text", "Ann"),
  field("agree", "checkbox", "Off", { options: ["Agreed"] }),
  field("fruit", "list", "", { options: ["Apple", "Pear"] }),
  field("size", "combo", "M", { options: ["S", "M"] }),
  field("locked", "text", "fixed", { read_only: true }),
  field("sig", "signature", ""),
];
const done = { id: "4", state: { kind: "done" }, progress: 1, outputs: ["/a/f-filled.pdf"] };

function backend(list: (args: { password?: string }) => unknown) {
  invoke.mockImplementation(async (cmd: string, args: { password?: string }) => {
    if (cmd === "forms_list_fields") return list(args);
    if (cmd === "forms_fill") return { job_id: "4" };
    if (cmd === "job_status") return done;
    throw new Error(`unexpected ${cmd}`);
  });
}

beforeEach(() => {
  invoke.mockReset();
});

describe("FillForm", () => {
  it("shows one control per field and sends only the changed values", async () => {
    backend(() => FIELDS);
    render(<FillForm path="/a/f.pdf" />);
    const name = (await screen.findByLabelText("person.name")) as HTMLInputElement;
    expect(name.value).toBe("Ann");
    expect((screen.getByLabelText("locked") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByLabelText("sig") as HTMLInputElement).disabled).toBe(true);
    const apply = screen.getByRole("button", { name: "Apply" }) as HTMLButtonElement;
    expect(apply.disabled).toBe(true);
    await userEvent.clear(name);
    await userEvent.type(name, "Zed");
    await userEvent.selectOptions(screen.getByLabelText("agree"), "Agreed");
    await userEvent.selectOptions(screen.getByLabelText("fruit"), "Pear");
    await userEvent.type(screen.getByLabelText("size"), "L");
    await userEvent.click(apply);
    const values = [
      { name: "person.name", value: "Zed" }, { name: "agree", value: "Agreed" },
      { name: "fruit", value: "Pear" }, { name: "size", value: "ML" },
    ];
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("forms_fill", { input: { input: "/a/f.pdf", values } }));
    expect(await screen.findByText("/a/f-filled.pdf")).toBeTruthy();
  });

  it("says so when the PDF has no form fields", async () => {
    backend(() => []);
    render(<FillForm path="/a/f.pdf" />);
    expect(await screen.findByText("This PDF has no form fields.")).toBeTruthy();
  });

  it("asks for the password of an encrypted form and fills with it", async () => {
    backend(({ password }) => (password === "pw" ? FIELDS.slice(0, 1) : Promise.reject(ENCRYPTED)));
    render(<FillForm path="/a/f.pdf" />);
    await userEvent.type(await screen.findByLabelText("Password for f.pdf"), "pw");
    await userEvent.click(screen.getByRole("button", { name: "Unlock and Retry" }));
    const name = await screen.findByLabelText("person.name");
    expect(invoke).toHaveBeenCalledWith("forms_list_fields", { path: "/a/f.pdf", password: "pw" });
    await userEvent.type(name, "!");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    const input = { input: "/a/f.pdf", password: "pw", values: [{ name: "person.name", value: "Ann!" }] };
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("forms_fill", { input }));
  });
});
