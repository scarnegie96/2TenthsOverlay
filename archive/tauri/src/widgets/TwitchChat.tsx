import { useEffect, useState, type ReactNode } from "react";
import { isTauri } from "../lib/bridge";
import { opt } from "../lib/config";
import { channelName, connectChat, type ChatMessage, type ChatStatus } from "../lib/twitch";
import { Empty, Panel } from "./Panel";
import type { WidgetProps } from "./registry";

/** Messages kept in memory, whatever the widget shows. */
const KEEP = 100;

/**
 * Twitch chat (experimental): a channel's live chat, read-only. Newest at the
 * bottom; older messages fade out after a while if asked, so it stays quiet
 * when chat is.
 */
export function TwitchChat({ config }: WidgetProps) {
  const raw = opt(config, "channel", "");
  // Browser preview: ?chat plays a made-up chat, on a made-up channel if none is set.
  const preview = !isTauri && new URLSearchParams(location.search).has("chat");
  const channel = channelName(raw) ?? (preview ? "tenths_racing" : null);
  const rows = opt(config, "rows", 10);
  const fadeAfter = opt(config, "fadeAfter", 60);
  const showEmotes = opt(config, "showEmotes", true);
  const { messages, status } = useChat(channel);
  const now = useNow(fadeAfter > 0 ? 1000 : 0);

  const shown = messages.slice(-rows).filter((m) => fadeAfter <= 0 || now - m.at < fadeAfter * 1000);
  const aside = channel ? (
    <span className={`tc-status is-${status}`} title={status === "connected" ? "Connected" : status}>#{channel}</span>
  ) : null;

  return (
    <Panel className="twitch-chat" title="Twitch chat" aside={aside}>
      {!channel ? (
        <Empty>{raw ? `"${raw}" isn't a Twitch channel name.` : "Set a channel in this widget's settings."}</Empty>
      ) : shown.length === 0 ? (
        <Empty>{status === "connected" ? "No new messages." : status === "error" ? "Can't reach Twitch chat." : "Joining chat…"}</Empty>
      ) : (
        <div className="tc-list" style={{ "--tc-rows": rows } as React.CSSProperties}>
          {shown.map((m) => (
            <div key={m.id} className={`tc-msg ${m.action ? "is-action" : ""}`}>
              <b className="tc-name" style={{ color: m.color ?? undefined }}>{m.name}</b>
              {m.action ? " " : ": "}
              <span className="tc-text">{showEmotes ? withEmotes(m) : m.text}</span>
            </div>
          ))}
        </div>
      )}
    </Panel>
  );
}

/** The text with Twitch emote images in place of their codes. */
function withEmotes(m: ChatMessage): ReactNode {
  if (m.emotes.length === 0) return m.text;
  // Emote positions count code points, not UTF-16 units.
  const chars = Array.from(m.text);
  const out: ReactNode[] = [];
  let at = 0;
  for (const e of m.emotes) {
    if (e.start < at || e.end >= chars.length) continue;
    if (e.start > at) out.push(chars.slice(at, e.start).join(""));
    const code = chars.slice(e.start, e.end + 1).join("");
    out.push(
      <img key={`${e.id}-${e.start}`} className="tc-emote" alt={code} title={code}
        src={`https://static-cdn.jtvnw.net/emoticons/v2/${encodeURIComponent(e.id)}/default/dark/1.0`} />,
    );
    at = e.end + 1;
  }
  if (at < chars.length) out.push(chars.slice(at).join(""));
  return out;
}

function useChat(channel: string | null) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [status, setStatus] = useState<ChatStatus>("connecting");
  useEffect(() => {
    setMessages([]);
    if (!channel) return;
    const add = (m: ChatMessage) => setMessages((all) => [...all.slice(-(KEEP - 1)), m]);
    // Browser preview: ?chat plays a made-up chat instead of connecting.
    if (!isTauri && new URLSearchParams(location.search).has("chat")) return demoChat(add, setStatus);
    return connectChat(channel, {
      message: add,
      deleted: (id) => setMessages((all) => all.filter((m) => m.id !== id)),
      cleared: (login) => setMessages((all) => (login ? all.filter((m) => m.login !== login) : [])),
      status: setStatus,
    });
  }, [channel]);
  return { messages, status };
}

/** Re-renders every `ms` (0: never), for fading old messages. */
function useNow(ms: number) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!ms) return;
    const t = window.setInterval(() => setNow(Date.now()), ms);
    return () => window.clearInterval(t);
  }, [ms]);
  return ms ? now : 0;
}

const DEMO: [string, string, string][] = [
  ["PitWallPete", "#1E90FF", "that pass into turn 1 was clean"],
  ["sim_sarah", "#FF69B4", "what's the fuel looking like?"],
  ["LateApex_", "#9ACD32", "box box box"],
  ["Grumpy_Marshal", "#FF4500", "track limits!!"],
  ["tenths_fan", "#DAA520", "gap to P3 is coming down"],
  ["CurbHopper", "#00CED1", "send it"],
];

function demoChat(add: (m: ChatMessage) => void, status: (s: ChatStatus) => void) {
  status("connected");
  let k = 0;
  const push = () => {
    const [name, color, text] = DEMO[k % DEMO.length];
    add({ id: `demo-${k}`, login: name.toLowerCase(), name, color, text, emotes: [], action: false, at: Date.now() });
    k++;
  };
  for (let i = 0; i < 5; i++) push();
  const t = window.setInterval(push, 2500);
  return () => window.clearInterval(t);
}
