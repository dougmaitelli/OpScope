import { createContext, useContext, useId, useRef } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import "./MarkdownContent.css";

const InsideLink = createContext(false);

function contentUrl(value: string, baseUrl: string, allowEmail: boolean): string | undefined {
  if (
    !value ||
    [...value].some((character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127)
  ) {
    return undefined;
  }
  try {
    const base = new URL(baseUrl);
    if (!["https:", "http:"].includes(base.protocol)) return undefined;
    const url = new URL(value, base);
    if (url.username || url.password) return undefined;
    return ["https:", "http:"].includes(url.protocol) || (allowEmail && url.protocol === "mailto:")
      ? url.href
      : undefined;
  } catch {
    return undefined;
  }
}

function ImageReference({ src, alt }: { src?: string; alt?: string }) {
  const insideLink = useContext(InsideLink);
  const label = `Image: ${alt || "Open image"}`;
  // Never load provider/user-controlled images inside the app. Also avoid nested
  // anchors for the common [![badge](image)](destination) Markdown pattern.
  return insideLink || !src ? (
    <span className="markdown-image-reference">{label}</span>
  ) : (
    <a className="markdown-image-reference" href={src} target="_blank" rel="noopener noreferrer">
      {label}
    </a>
  );
}

export function MarkdownContent({ content, baseUrl }: { content: string; baseUrl: string }) {
  const container = useRef<HTMLDivElement>(null);
  const prefix = `markdown-${useId()}-`;
  return (
    <div className="markdown-content" ref={container}>
      <Markdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[
          rehypeRaw,
          [
            rehypeSanitize,
            {
              ...defaultSchema,
              clobberPrefix: prefix,
              strip: [
                ...(defaultSchema.strip ?? []),
                "style",
                "iframe",
                "object",
                "embed",
                "form",
                "textarea",
                "select",
                "button",
              ],
            },
          ],
        ]}
        urlTransform={(url, key) => {
          if (key === "href" && url.startsWith("#")) return url;
          return contentUrl(url, baseUrl, key === "href");
        }}
        components={{
          a: ({ href, children, title, id }) => {
            const destination = href ? contentUrl(href, baseUrl, true) : undefined;
            return destination ? (
              <a
                id={id}
                href={destination}
                title={title}
                target="_blank"
                rel="noopener noreferrer"
                onClick={(event) => {
                  if (!href?.startsWith("#")) return;
                  let fragment: string;
                  try {
                    fragment = decodeURIComponent(href.slice(1));
                  } catch {
                    return;
                  }
                  const target = document.getElementById(`${prefix}${fragment}`);
                  if (target && container.current?.contains(target)) {
                    event.preventDefault();
                    target.scrollIntoView({ block: "nearest" });
                  }
                }}
              >
                <InsideLink.Provider value={true}>{children}</InsideLink.Provider>
              </a>
            ) : (
              <span>{children}</span>
            );
          },
          img: ({ src, alt }) => <ImageReference src={src} alt={alt} />,
          table: ({ children }) => (
            <div className="markdown-table">
              <table>{children}</table>
            </div>
          ),
        }}
      >
        {content}
      </Markdown>
    </div>
  );
}
