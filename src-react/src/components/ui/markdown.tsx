import { memo } from "react";
import ReactMarkdown from "react-markdown";
import type { Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "@/lib/utils";

// Element-level styling mapped onto the nexus sand theme. Kept here so any
// surface that renders Markdown shares one consistent look.
const components: Components = {
  h1: ({ children }) => (
    <h1 className="mb-3 mt-5 text-[19px] font-extrabold tracking-[-.01em] text-nexus-ink first:mt-0">
      {children}
    </h1>
  ),
  h2: ({ children }) => (
    <h2 className="mb-2.5 mt-5 text-[16px] font-bold tracking-[-.01em] text-nexus-ink first:mt-0">
      {children}
    </h2>
  ),
  h3: ({ children }) => (
    <h3 className="mb-2 mt-4 text-[14px] font-bold text-nexus-body first:mt-0">{children}</h3>
  ),
  h4: ({ children }) => (
    <h4 className="mb-1.5 mt-3.5 text-[13px] font-bold text-nexus-body first:mt-0">{children}</h4>
  ),
  p: ({ children }) => (
    <p className="my-2.5 text-[13px] leading-[1.7] text-[#4a4138] first:mt-0 last:mb-0">
      {children}
    </p>
  ),
  a: ({ children, href }) => (
    <a
      href={href}
      target="_blank"
      rel="noreferrer noopener"
      className="font-medium text-nexus-accent underline decoration-nexus-accent/40 underline-offset-2 hover:decoration-nexus-accent"
    >
      {children}
    </a>
  ),
  strong: ({ children }) => <strong className="font-bold text-nexus-ink">{children}</strong>,
  em: ({ children }) => <em className="italic">{children}</em>,
  ul: ({ children }) => (
    <ul className="my-2.5 list-disc space-y-1 pl-5 text-[13px] leading-[1.7] text-[#4a4138]">
      {children}
    </ul>
  ),
  ol: ({ children }) => (
    <ol className="my-2.5 list-decimal space-y-1 pl-5 text-[13px] leading-[1.7] text-[#4a4138]">
      {children}
    </ol>
  ),
  li: ({ children }) => <li className="marker:text-[#b3a999]">{children}</li>,
  blockquote: ({ children }) => (
    <blockquote className="my-3 border-l-[3px] border-nexus-accent/50 bg-[#f8f3ea] py-0.5 pl-3.5 pr-2 text-[#6a6055]">
      {children}
    </blockquote>
  ),
  hr: () => <hr className="my-4 border-none border-t border-nexus-border2" />,
  code: ({ className, children }) => {
    const isBlock = /language-/.test(className ?? "");
    if (isBlock) {
      return <code className="font-mono text-[12px] leading-[1.6]">{children}</code>;
    }
    return (
      <code className="rounded-[5px] bg-[#efe6da] px-1.5 py-0.5 font-mono text-[12px] text-[#7a5c4a]">
        {children}
      </code>
    );
  },
  pre: ({ children }) => (
    <pre className="my-3 overflow-auto rounded-[10px] border border-nexus-panel bg-[#f8f3ea] px-4 py-3 text-[#4a4138]">
      {children}
    </pre>
  ),
  table: ({ children }) => (
    <div className="my-3 overflow-auto">
      <table className="w-full border-collapse text-[12.5px]">{children}</table>
    </div>
  ),
  thead: ({ children }) => <thead className="bg-[#f0e8db]">{children}</thead>,
  th: ({ children }) => (
    <th className="border border-nexus-border2 px-2.5 py-1.5 text-left font-bold text-nexus-body">
      {children}
    </th>
  ),
  td: ({ children }) => (
    <td className="border border-nexus-border2 px-2.5 py-1.5 text-[#4a4138]">{children}</td>
  ),
  img: ({ src, alt }) => (
    <img src={src} alt={alt} className="my-3 max-w-full rounded-[8px] border border-nexus-panel" />
  ),
};

export const Markdown = memo(function Markdown({
  children,
  className,
}: {
  children: string;
  className?: string;
}) {
  return (
    <div className={cn("text-[13px]", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {children}
      </ReactMarkdown>
    </div>
  );
});
