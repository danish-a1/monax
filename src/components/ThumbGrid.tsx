import { useEffect, useRef } from "react";

export interface PageItem { key: number; page: number; rotate: number }

interface Props {
  items: PageItem[];
  thumbs: Record<number, string>;
  selected: Set<number>;
  onToggle: (key: number) => void;
  onMove: (from: number, to: number) => void;
  /** Called when a card for `page` scrolls near the viewport (may repeat). */
  onVisible: (page: number) => void;
}

/** Page cards: click to select, drag to reorder (native HTML5 DnD). Rotation is a preview only. */
export default function ThumbGrid({ items, thumbs, selected, onToggle, onMove, onVisible }: Props) {
  const from = useRef<number | null>(null);
  const list = useRef<HTMLUListElement>(null);

  useEffect(() => {
    const io = new IntersectionObserver((entries) => {
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        io.unobserve(e.target);
        onVisible(Number((e.target as HTMLElement).dataset.page));
      }
    }, { rootMargin: "200px" });
    list.current?.querySelectorAll("li").forEach((li) => io.observe(li));
    return () => io.disconnect();
  }, [items, onVisible]);

  return (
    <ul className="grid" ref={list}>
      {items.map((it, i) => (
        <li
          key={it.key}
          data-page={it.page}
          draggable
          className={selected.has(it.key) ? "card selected" : "card"}
          onDragStart={(e) => {
            from.current = i;
            e.dataTransfer?.setData("text/plain", String(i));
          }}
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            e.preventDefault();
            if (from.current !== null && from.current !== i) onMove(from.current, i);
            from.current = null;
          }}
        >
          <button type="button" aria-pressed={selected.has(it.key)} aria-label={`Select page ${it.page + 1}`} onClick={() => onToggle(it.key)}>
            <span className="frame">
              {thumbs[it.page] && (
                <img src={thumbs[it.page]} alt={`Page ${it.page + 1}`} draggable={false} style={{ transform: `rotate(${it.rotate}deg)` }} />
              )}
            </span>
            <span className="num">{it.page + 1}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
