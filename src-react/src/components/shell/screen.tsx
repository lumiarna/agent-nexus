import type { ReactNode } from "react";

/** Standard scrollable page body: centered column with the prototype's
 *  24/32/60 padding. `overflow-y: scroll` always renders the 11px scrollbar
 *  track (WebKit reserves it even with no overflow; `scrollbar-gutter` is
 *  unreliable in the macOS WKWebView), so switching between a scrolling page
 *  (Skill) and a non-scrolling one (Prompt) doesn't shift the centered column.
 *  Pages with bespoke layouts (Session) don't use this. */
export function ScreenScroll({
  children,
  maxWidth = "1480px",
}: {
  children: ReactNode;
  maxWidth?: string;
}) {
  return (
    <div className="min-h-0 flex-1 overflow-x-auto overflow-y-scroll">
      <div className="mx-auto px-8 pb-[60px] pt-6" style={{ maxWidth }}>
        {children}
      </div>
    </div>
  );
}
