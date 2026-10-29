import { useCallback, useEffect, useRef, useState } from "react";
import JobStatus, { useJob } from "../components/JobStatus";
import ThumbGrid, { PageItem } from "../components/ThumbGrid";
import PasswordPrompt, { passwordNeed } from "../components/PasswordPrompt";
import { callTool, pagesInfo, pagesThumbnail } from "../lib/invoke";

const THUMB_PX = 160;

/** Per-file thumbnail state; `live` turns false when the file changes or the panel unmounts. */
interface Session { live: boolean; asked: Set<number>; urls: string[] }

export default function Organize({ path }: { path: string }) {
  const [items, setItems] = useState<PageItem[]>([]);
  const [thumbs, setThumbs] = useState<Record<number, string>>({});
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [loadError, setLoadError] = useState<string | null>(null);
  /** Opens an encrypted file; asked for when it needs one (App keys this panel by path). */
  const [password, setPassword] = useState<string | undefined>(undefined);
  const [tries, setTries] = useState(0); // the same wrong password twice still retries
  const { job, error, busy, run, cancel } = useJob();
  const session = useRef<Session | null>(null);

  useEffect(() => {
    const s: Session = { live: true, asked: new Set(), urls: [] };
    session.current = s;
    setItems([]); setThumbs({}); setSelected(new Set()); setLoadError(null);
    pagesInfo(path, password).then(
      ({ page_count }) => { if (s.live) setItems(Array.from({ length: page_count }, (_, i) => ({ key: i, page: i, rotate: 0 }))); },
      (e) => { if (s.live) setLoadError(String(e)); },
    );
    return () => { s.live = false; s.urls.forEach((u) => URL.revokeObjectURL(u)); };
  }, [path, password, tries]);

  // Thumbnails load lazily, once per page, as cards scroll into view.
  const want = useCallback((page: number) => {
    const s = session.current;
    if (!s?.live || s.asked.has(page)) return;
    s.asked.add(page);
    pagesThumbnail(path, page, THUMB_PX, password).then(
      (bytes) => {
        if (!s.live) return;
        const url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        s.urls.push(url);
        setThumbs((t) => ({ ...t, [page]: url }));
      },
      (e) => { if (s.live) setLoadError(String(e)); },
    );
  }, [path, password]);

  const toggle = (key: number) => setSelected((s) => { const n = new Set(s); if (!n.delete(key)) n.add(key); return n; });
  const move = (from: number, to: number) => setItems((cur) => {
    const next = [...cur];
    next.splice(to, 0, next.splice(from, 1)[0]);
    return next;
  });
  const rotate = (deg: number) => setItems((cur) => cur.map((it) => (selected.has(it.key) ? { ...it, rotate: (it.rotate + deg + 360) % 360 } : it)));
  const remove = () => { setItems((cur) => cur.filter((it) => !selected.has(it.key))); setSelected(new Set()); };
  const plan = items.map((it) => ({ page: it.page, rotate: it.rotate }));
  const apply = () => void run(() => callTool("organize", { input: path, ...(password ? { password } : {}), plan }));
  const need = passwordNeed(loadError);
  const none = selected.size === 0;

  return (
    <section>
      <h2>Organize</h2>
      <div className="toolbar">
        <button type="button" disabled={none} onClick={() => rotate(-90)}>Rotate left</button>
        <button type="button" disabled={none} onClick={() => rotate(90)}>Rotate right</button>
        <button type="button" disabled={none} onClick={remove}>Delete selected</button>
        <button type="button" className="primary" disabled={items.length === 0 || busy} onClick={apply}>Apply</button>
      </div>
      {loadError && <p role="alert" className="error">{loadError}</p>}
      {need && <PasswordPrompt files={[path]} wrong={need === "wrong"} onSubmit={([p]) => { setPassword(p); setTries((t) => t + 1); }} />}
      <ThumbGrid items={items} thumbs={thumbs} selected={selected} onToggle={toggle} onMove={move} onVisible={want} />
      <JobStatus job={job} error={error} onCancel={(id) => void cancel(id)} />
    </section>
  );
}
