import { useState, useRef, useEffect, type ReactNode } from "react";
import { Info } from "@phosphor-icons/react";
import { cn } from "./ui";

/**
 * Small `i`-in-a-circle Phosphor info icon. On hover it shows the `text`
 * (single-line `title=""` fallback for short hints) and on click toggles
 * a richer popover for multi-line explainers.
 *
 * Use this next to a section header to explain what a whole subsection
 * does (3-4 sentences), or next to a parameter label for a 1-2 sentence
 * meaning + range explainer.
 *
 * Implementation note: we render an absolutely-positioned popover that
 * shows on hover (via CSS) OR while `open` (toggled by click), so a user
 * can click to pin it open and read the full text instead of relying on
 * a tooltip that vanishes the moment the pointer moves.
 */
export function InfoTooltip({
  text,
  className,
  size = 12,
  label,
}: {
  /** The explainer text. Can be a single string or any ReactNode for
   *  richer formatting (e.g. a heading + paragraph). */
  text: ReactNode;
  className?: string;
  /** Phosphor icon size in pixels. Defaults to 12 (parameter-level);
   *  use 14 for section-header variants. */
  size?: number;
  /** Optional aria-label override. Defaults to "More info". */
  label?: string;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLSpanElement>(null);

  // Click-outside to close so a pinned popover doesn't trap the user.
  useEffect(() => {
    if (!open) return;
    const onDocClick = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [open]);

  const titleStr = typeof text === "string" ? text : undefined;

  return (
    <span
      ref={rootRef}
      className={cn(
        "relative inline-flex items-center align-middle group/info",
        className
      )}
    >
      <button
        type="button"
        aria-label={label ?? "More info"}
        aria-expanded={open}
        title={titleStr}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((v) => !v);
        }}
        className={cn(
          "inline-flex items-center justify-center rounded-full",
          "text-muted-foreground/60 hover:text-primary",
          "focus:outline-none focus-visible:ring-2 focus-visible:ring-primary/60",
          "transition-colors"
        )}
      >
        <Info size={size} weight="regular" />
      </button>
      <span
        role="tooltip"
        className={cn(
          // Positioned just below the icon. We use the right edge so
          // popovers placed near the right of a row don't overflow.
          "absolute top-full left-1/2 -translate-x-1/2 mt-2 z-50",
          "w-64 max-w-[80vw] p-3 rounded-xl",
          "bg-card border border-border shadow-xl",
          "text-[11px] leading-relaxed text-foreground normal-case tracking-normal",
          "pointer-events-none",
          // Hover-show via the group selector, OR click-pinned via `open`.
          open
            ? "opacity-100 pointer-events-auto"
            : "opacity-0 group-hover/info:opacity-100"
        )}
      >
        {text}
      </span>
    </span>
  );
}
