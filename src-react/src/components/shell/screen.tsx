import type { ReactNode } from "react";

/** Standard scrollable page body: centered column with the prototype's
 *  24/32/60 padding. Pages with bespoke layouts (Session) don't use this. */
export function ScreenScroll({
  children,
  maxWidth = "1480px",
}: {
  children: ReactNode;
  maxWidth?: string;
}) {
  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <div className="mx-auto px-8 pb-[60px] pt-6" style={{ maxWidth }}>
        {children}
      </div>
    </div>
  );
}
