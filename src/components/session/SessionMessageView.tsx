import type { SessionMessage } from "../../api/client";
import { Time } from "../Time";
import { toast } from "../use-toast";
import { copyText, messageRole } from "./session-content";

const MESSAGE_COLLAPSE_THRESHOLD = 3000;
const MESSAGE_COLLAPSED_LENGTH = 1500;

interface Props {
  message: SessionMessage;
  index: number;
  targeted: boolean;
  expanded: boolean;
  onToggleExpanded: (index: number) => void;
}

/** Renders one read-only transcript entry and owns its local copy feedback. */
export function SessionMessageView({
  message,
  index,
  targeted,
  expanded,
  onToggleExpanded,
}: Props) {
  const isLong = message.content.length > MESSAGE_COLLAPSE_THRESHOLD;
  const collapsed = isLong && !expanded;
  const display = collapsed ? `${message.content.slice(0, MESSAGE_COLLAPSED_LENGTH)}…` : message.content;

  const copy = async () => {
    try {
      await copyText(message.content);
      toast({ kind: "success", title: "已复制消息内容" });
    } catch {
      toast({ kind: "error", title: "无法复制消息内容" });
    }
  };

  return (
    <article
      className={`asb-session-message is-${message.role.toLowerCase()}${targeted ? " is-target" : ""}`}
      data-index={index}
    >
      <header>
        <span>{messageRole(message.role)}</span>
        <span className="asb-session-message-time">
          {message.at ? <Time iso={message.at} /> : null}
        </span>
        <button type="button" className="asb-session-message-copy" onClick={() => void copy()}>
          复制
        </button>
      </header>
      <pre>{display}</pre>
      {isLong && (
        <button
          type="button"
          className="asb-session-message-toggle"
          aria-expanded={expanded}
          onClick={() => onToggleExpanded(index)}
        >
          {expanded ? "收起" : `展开完整内容（约 ${Math.round(message.content.length / 1000)}k 字符）`}
        </button>
      )}
    </article>
  );
}
