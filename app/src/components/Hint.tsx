import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/**
 * What an option does, shown on hover or keyboard focus instead of a paragraph under it. The
 * bubble is drawn over the whole window (the scrolling content would cut it at the menu), kept
 * inside it. Inside a label, a click on the "?" does not toggle the label's control.
 */
export function Hint({ text }: { text: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  const tip = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<DOMRect | null>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  // Over the "?" and centred on it, else under it; never past the window's edges.
  useLayoutEffect(() => {
    if (!at || !tip.current) return;
    const { width, height } = tip.current.getBoundingClientRect();
    const left = Math.min(Math.max(8, at.left + at.width / 2 - width / 2), window.innerWidth - width - 8);
    const top = at.top - height - 8 >= 8 ? at.top - height - 8 : at.bottom + 8;
    setPos({ left, top });
  }, [at]);

  const show = () => ref.current && setAt(ref.current.getBoundingClientRect());
  const hide = () => {
    setAt(null);
    setPos(null);
  };
  return (
    <>
      <span
        ref={ref}
        className="hint"
        tabIndex={0}
        role="img"
        aria-label={text}
        onMouseEnter={show}
        onMouseLeave={hide}
        onFocus={show}
        onBlur={hide}
        onClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
        }}
      >
        ?
      </span>
      {at &&
        createPortal(
          <div ref={tip} className="hint-tip" role="tooltip" style={pos ?? { left: 0, top: 0, visibility: "hidden" }}>
            {text}
          </div>,
          document.body,
        )}
    </>
  );
}
