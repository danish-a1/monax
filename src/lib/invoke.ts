import { invoke } from "@tauri-apps/api/core";
import { TOOLS } from "../tools";
import type { PageBoxes } from "./coords";

export type JobState = { kind: "queued" | "running" | "done" | "cancelled" } | { kind: "failed"; message: string };
/** `notes`: what the user should know about the result (not errors); absent when none. */
export interface Job { id: string; state: JobState; progress: number; outputs: string[]; notes?: string[] }

/** Runs a registered tool's command; the backend returns immediately with a job id. */
export function callTool(id: string, input: object): Promise<{ job_id: string }> {
  const tool = TOOLS.find((t) => t.id === id);
  if (!tool) return Promise.reject(new Error(`unknown tool: ${id}`));
  return invoke(tool.command, { input });
}

/** Core's messages for a PDF that needs a password, and for a rejected one. */
export const ENCRYPTED = "encrypted input: a password is required";
export const WRONG_PASSWORD = "wrong password";
const pw = (password?: string) => (password ? { password } : {});

/** Page count and each page's boxes and /Rotate. */
export const pagesInfo = (path: string, password?: string) =>
  invoke<{ page_count: number; pages: PageBoxes[] }>("pages_info", { path, ...pw(password) });
/** PNG of one page as displayed, longer side `maxPx` (16..=4096). */
export const pagesThumbnail = (path: string, page: number, maxPx: number, password?: string) =>
  invoke<ArrayBuffer>("pages_thumbnail", { path, page, max_px: maxPx, ...pw(password) });
/** A form field as the core lists it: rect in PDF user space points, page 0-based. */
export interface FormField {
  name: string;
  kind: "text" | "checkbox" | "radio" | "combo" | "list" | "button" | "signature";
  page: number | null;
  rect: { left: number; bottom: number; right: number; top: number } | null;
  value: string;
  options?: string[];
  read_only: boolean;
}
export const listFields = (path: string, password?: string) =>
  invoke<FormField[]>("forms_list_fields", { path, ...pw(password) });
/** Values a select offers, from a backend command (installed OCR languages). */
export const choices = (command: string) => invoke<string[]>(command);
/** Whether the bundled WordCraft is present (Office to PDF needs it). */
export const officeAvailable = () => invoke<boolean>("office_available");
export const jobStatus = (jobId: string) => invoke<Job | null>("job_status", { job_id: jobId });
export const jobCancel = (jobId: string) => invoke<void>("job_cancel", { job_id: jobId });

export type McpStatus =
  | { status: "starting" }
  | { status: "running"; url: string; token: string; port_changed: boolean }
  | { status: "failed"; reason: string };
export const mcpInfo = () => invoke<McpStatus>("mcp_info");

export const isTerminal = (j: Job) => ["done", "failed", "cancelled"].includes(j.state.kind);

/** Polls until a terminal state or an unknown id (null). Returns a stop function. */
export function pollJob(jobId: string, onUpdate: (j: Job | null, error?: string) => void, intervalMs = 250): () => void {
  let stopped = false;
  const tick = async () => {
    if (stopped) return;
    let job: Job | null;
    try {
      job = await jobStatus(jobId);
    } catch (e) {
      if (!stopped) onUpdate(null, String(e instanceof Error ? e.message : e));
      return;
    }
    if (stopped) return;
    onUpdate(job);
    if (job && !isTerminal(job)) setTimeout(tick, intervalMs);
  };
  void tick();
  return () => {
    stopped = true;
  };
}
