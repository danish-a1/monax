import { useEffect, useRef, useState } from "react";
import { pagesThumbnail } from "../lib/invoke";

interface Props {
  path: string;
  password?: string;
  /** 0-based page. */
  page: number;
  /** Displayed size, CSS pixels. */
  width: number;
  height: number;
  onError: (message: string) => void;
  /** A render arrived (clears an earlier failure). */
  onRendered?: () => void;
}

/** The backend renders at most this many pixels on the longer side: core `pdfium::VIEW_MAX_PX`. */
const MAX_PX = 4096;
/** Wait this long after a zoom or resize before asking for a new render. */
const DEBOUNCE_MS = 150;

/**
 * One page as displayed (CropBox, /Rotate, annotations, fields), rendered by the backend's
 * pdfium at the device-pixel size it is drawn at. The previous image stays, stretched, until
 * the new one arrives; each replaced image is released.
 */
export default function PdfCanvas({ path, password, page, width, height, onError, onRendered }: Props) {
  const [url, setUrl] = useState<string | null>(null);
  const current = useRef<string | null>(null);
  // ponytail: one image up to 4096 px; tile the visible part if deep zoom must stay sharp.
  const px = Math.min(MAX_PX, Math.max(16, Math.round(Math.max(width, height) * (window.devicePixelRatio || 1))));
  const report = useRef({ onError, onRendered });
  report.current = { onError, onRendered };

  useEffect(() => {
    let live = true;
    const timer = setTimeout(() => {
      pagesThumbnail(path, page, px, password).then(
        (bytes) => {
          if (!live) return;
          const next = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
          if (current.current) URL.revokeObjectURL(current.current);
          current.current = next;
          setUrl(next);
          report.current.onRendered?.();
        },
        (e) => { if (live) report.current.onError(String(e)); },
      );
    }, DEBOUNCE_MS);
    return () => { live = false; clearTimeout(timer); };
  }, [path, password, page, px]);

  useEffect(() => () => { if (current.current) URL.revokeObjectURL(current.current); }, []);

  return (
    <div className="canvas" style={{ width, height }}>
      {url && <img src={url} alt={`Page ${page + 1}`} width={width} height={height} draggable={false} />}
    </div>
  );
}
