import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

export function Markdown({ text, allowImages = true }: { text: string; allowImages?: boolean }) {
  return <div className="markdown"><ReactMarkdown remarkPlugins={[remarkGfm]} components={{ ...(allowImages ? {} : { img: () => null }), a: ({ children, ...props }) => <a {...props} target="_blank" rel="noopener noreferrer">{children}</a> }}>{text}</ReactMarkdown></div>;
}
