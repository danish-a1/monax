import { useEffect, useRef, useState } from "react";
import AnnotLayer from "../components/AnnotLayer";
import JobStatus, { useJob } from "../components/JobStatus";
import PasswordPrompt, { passwordNeed } from "../components/PasswordPrompt";
import PdfCanvas from "../components/PdfCanvas";
import { type Drawn, makeMark, type Pending, type Style, type Tool } from "../lib/annots";
import { displayedSize, type PageBoxes } from "../lib/coords";
import { callTool, isTerminal, pagesInfo } from "../lib/invoke";
import MarkList from "./MarkList";
import ViewerBar, { ZOOM_STEPS } from "./ViewerBar";

/** CSS pixels per point at 100%. */
const PT_PX = 4 / 3;
const [MIN_ZOOM, MAX_ZOOM] = [ZOOM_STEPS[0], ZOOM_STEPS[ZOOM_STEPS.length - 1]];
/** A text-entry control, which keeps its own keys. */
const textEntry = (t: Element) => ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName)
  || t.closest('[contenteditable]:not([contenteditable="false"])') !== null;
/** What a finished job applied: its marks leave the list, the status names the output. */
interface Applying { ids: Set<number>; saved: (out: string) => string }

/** Shows one page at a time; draws annotations and redaction boxes, then saves them as new files. */
export default function Viewer({ path }: { path: string }) {
  const [pages, setPages] = useState<PageBoxes[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  /** Opens an encrypted file; asked for when it needs one (App keys this panel by path). */
  const [password, setPassword] = useState<string | undefined>(undefined);
  const [tries, setTries] = useState(0); // the same wrong password twice still retries
  const [page, setPage] = useState(0);
  const [zoom, setZoom] = useState<number | null>(null); // null: fit width
  const [fitPx, setFitPx] = useState(0);
  const [tool, setTool] = useState<Tool>("select");
  const [style, setStyle] = useState<Style>({ color: "#ffd60a", width: 2, text: "", fontSize: 12 });
  const [items, setItems] = useState<Pending[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const applying = useRef<Applying | null>(null);
  const nextId = useRef(1);
  const view = useRef<HTMLDivElement>(null);
  const panel = useRef<HTMLElement>(null);
  const { job, error, busy, run, cancel } = useJob();

  useEffect(() => {
    let live = true;
    setPages([]); setLoadError(null);
    pagesInfo(path, password).then(
      (info) => { if (live) setPages(info.pages); },
      (e) => { if (live) setLoadError(String(e)); },
    );
    return () => { live = false; };
  }, [path, password, tries]);

  useEffect(() => {
    const measure = () => setFitPx(view.current?.clientWidth ?? 0);
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, []);

  useEffect(() => {
    const a = applying.current;
    if (!a || !job || !isTerminal(job)) return;
    applying.current = null;
    if (job.state.kind !== "done") return; // failed or cancelled: the marks stay to retry
    setItems((cur) => cur.filter((i) => !a.ids.has(i.id)));
    setSelected(null);
    setSaved(a.saved(job.outputs[0]));
  }, [job]);

  const count = pages.length;
  const go = (p: number) => setPage(Math.min(Math.max(p, 0), Math.max(count - 1, 0)));
  const remove = (id: number) => { setItems((cur) => cur.filter((i) => i.id !== id)); setSelected(null); };

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      // Viewer keys: with nothing focused (body) or anywhere in this panel, except in text entry.
      const t = e.target;
      const mine = t === document.body || (t instanceof Element && panel.current?.contains(t) && !textEntry(t));
      if (!mine) return;
      if (["ArrowRight", "ArrowDown", "PageDown"].includes(e.key)) go(page + 1);
      else if (["ArrowLeft", "ArrowUp", "PageUp"].includes(e.key)) go(page - 1);
      else if ((e.key === "Delete" || e.key === "Backspace") && selected !== null) remove(selected);
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const geom = pages[page];
  const shown = geom && displayedSize(geom);
  const z = zoom ?? (shown && fitPx > 0 ? Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, fitPx / (shown.width * PT_PX))) : 1);
  const draw = (d: Drawn) => {
    if (tool === "select") return;
    const mark = makeMark(tool, d, style);
    if (typeof mark === "string") return setNotice(mark);
    setNotice(null);
    setItems((cur) => [...cur, { id: nextId.current++, page, mark }]);
  };
  const select = (id: number) => {
    setSelected(id);
    const it = items.find((i) => i.id === id);
    if (it) go(it.page);
  };
  /** Runs `tool` on the marks `pick` takes (as the input key `key`); they leave the list once done. */
  const apply = (tool: string, saved: Applying["saved"], key: string, pick: (i: Pending) => object | null) => {
    const picked = items.flatMap((i) => { const v = pick(i); return v ? [{ id: i.id, v }] : []; });
    applying.current = { ids: new Set(picked.map((p) => p.id)), saved };
    setSaved(null);
    void run(() => callTool(tool, { input: path, ...(password ? { password } : {}), [key]: picked.map((p) => p.v) }));
  };
  const save = () => apply("viewer", (out) => `Annotations saved to ${out}`, "annots", ({ page, mark }) => (mark.type === "redact" ? null : { page, annot: mark }));
  const redact = () => apply("redact", (out) => `Redacted copy saved to ${out} (this view still shows the original)`, "boxes", ({ page, mark }) => (mark.type === "redact" ? { page, rect: mark.rect } : null));
  const need = passwordNeed(loadError);

  return (
    <section className="viewer" ref={panel}>
      <h2>View & Annotate</h2>
      <ViewerBar page={page} count={count} onPage={go} zoom={z} onZoom={setZoom} tool={tool} onTool={setTool} style={style} onStyle={setStyle} />
      {loadError && <p role="alert" className="error">{loadError}</p>}
      {need && <PasswordPrompt files={[path]} wrong={need === "wrong"} onSubmit={([p]) => { setPassword(p); setTries((t) => t + 1); }} />}
      {notice && <p role="status" className="error">{notice}</p>}
      {saved && <p role="status">{saved}</p>}
      <div className="viewport" ref={view} tabIndex={0} role="region" aria-label="Page View">
        {shown && (
          <div className="page" style={{ width: shown.width * z * PT_PX, height: shown.height * z * PT_PX }}>
            <PdfCanvas key={page} path={path} password={password} page={page}
              width={shown.width * z * PT_PX} height={shown.height * z * PT_PX} onError={setLoadError} onRendered={() => setLoadError(null)} />
            <AnnotLayer page={geom} width={shown.width * z * PT_PX} height={shown.height * z * PT_PX} tool={tool}
              items={items.filter((i) => i.page === page)} selected={selected} onDraw={draw} onSelect={select} />
          </div>
        )}
      </div>
      <MarkList items={items} selected={selected} busy={busy} onSelect={select} onRemove={remove} onSave={save} onRedact={redact} />
      <JobStatus job={job} error={error} onCancel={(id) => void cancel(id)} />
    </section>
  );
}
