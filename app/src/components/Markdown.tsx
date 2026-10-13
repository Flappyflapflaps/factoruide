import { Fragment, type ReactNode } from "react";
import { api } from "../api";

/**
 * The Markdown of the project's own files (headings, paragraphs, lists, bold, code, links),
 * drawn as elements, never as HTML. Links to the project open in the browser; others are text.
 */
export function Markdown({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  let list: string[] = [];
  let paragraph: string[] = [];
  const flush = () => {
    if (paragraph.length) blocks.push(<p key={blocks.length}>{inline(paragraph.join(" "))}</p>);
    if (list.length)
      blocks.push(
        <ul key={blocks.length}>
          {list.map((item, i) => (
            <li key={i}>{inline(item)}</li>
          ))}
        </ul>,
      );
    paragraph = [];
    list = [];
  };
  // Windows line ends too (a checkout on Windows turns the file's into CRLF).
  for (const line of text.split(/\r?\n/)) {
    const heading = /^(#{1,4})\s+(.*)$/.exec(line);
    const item = /^\s*[-*]\s+(.*)$/.exec(line);
    if (heading) {
      flush();
      // "#" and "##" are the page's sections, "###" their parts.
      const level = Math.min(4, Math.max(2, heading[1].length));
      const Tag = `h${level}` as "h2" | "h3" | "h4";
      blocks.push(<Tag key={blocks.length}>{inline(heading[2])}</Tag>);
    } else if (item) {
      if (paragraph.length) flush();
      list.push(item[1]);
    } else if (!line.trim()) {
      flush();
    } else if (list.length && /^\s+/.test(line)) {
      // A list item going on over the next line.
      list[list.length - 1] += ` ${line.trim()}`;
    } else {
      if (list.length) flush();
      paragraph.push(line.trim());
    }
  }
  flush();
  return <div className="markdown">{blocks}</div>;
}

/** Bold, code and links within a line. */
function inline(text: string): ReactNode {
  const parts: ReactNode[] = [];
  const pattern = /\*\*(.+?)\*\*|`([^`]+)`|\[([^\]]+)\]\(([^)]+)\)/g;
  let last = 0;
  for (let m = pattern.exec(text); m; m = pattern.exec(text)) {
    if (m.index > last) parts.push(text.slice(last, m.index));
    const key = parts.length;
    if (m[1]) parts.push(<strong key={key}>{m[1]}</strong>);
    else if (m[2]) parts.push(<code key={key}>{m[2]}</code>);
    else parts.push(<Link key={key} url={m[4]} label={m[3]} />);
    last = m.index + m[0].length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return parts.map((p, i) => <Fragment key={i}>{p}</Fragment>);
}

/** The project's base address. */
export const REPO = "https://github.com/brouznouf/factoruide/";
/** Pages that open in the browser (as the app's `open_link` allows): the project's, and the community tools. */
const OPENS = [REPO, "https://github.com/Questie/QuestieTrace", "https://questie.dev/trace", "https://www.wowhead.com/"];

export function Link({ url, label }: { url: string; label: ReactNode }) {
  // Relative links of the repository's files point into it.
  const full = /^https?:/.test(url) ? url : `${REPO}blob/main/${url.replace(/^\.\//, "")}`;
  if (!OPENS.some((o) => full.startsWith(o))) return <span title={full}>{label}</span>;
  return (
    <button className="link" onClick={() => api.openLink(full).catch(() => {})} title={full}>
      {label}
    </button>
  );
}
