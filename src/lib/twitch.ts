// Read-only Twitch chat over Twitch's IRC WebSocket. No account or token is
// needed to read a public channel: Twitch lets anonymous "justinfan" logins
// join and listen. Messages arrive with IRCv3 tags (name colour, display name,
// emotes), and moderation (a deleted message, a timeout or ban) removes what
// was shown.

export interface ChatEmote {
  id: string;
  /** Code point positions in the text, inclusive. */
  start: number;
  end: number;
}

export interface ChatMessage {
  id: string;
  /** Lower-case login, for timeouts and bans. */
  login: string;
  name: string;
  color: string | null;
  text: string;
  emotes: ChatEmote[];
  /** "/me" messages. */
  action: boolean;
  /** Date.now() when it arrived. */
  at: number;
}

export type ChatStatus = "connecting" | "connected" | "reconnecting" | "error";

export interface ChatHandlers {
  message: (m: ChatMessage) => void;
  /** A message removed by a moderator. */
  deleted: (id: string) => void;
  /** Everything from one user removed (timeout or ban), or the whole chat cleared (login null). */
  cleared: (login: string | null) => void;
  status: (s: ChatStatus) => void;
}

const URL = "wss://irc-ws.chat.twitch.tv:443";

/** Valid Twitch login: 3–25 letters, digits or underscores. */
export function channelName(raw: string): string | null {
  const c = raw.trim().replace(/^#/, "").replace(/^https?:\/\/(www\.)?twitch\.tv\//i, "").split(/[/?#]/)[0].toLowerCase();
  return /^[a-z0-9_]{3,25}$/.test(c) ? c : null;
}

/** IRCv3 tag values escape ; space \ CR LF. */
function unescapeTag(v: string): string {
  return v.replace(/\\(.)/g, (_, c: string) => ({ ":": ";", s: " ", "\\": "\\", r: "\r", n: "\n" })[c] ?? c);
}

export interface IrcLine {
  tags: Record<string, string>;
  prefix: string | null;
  command: string;
  params: string[];
}

export function parseLine(line: string): IrcLine | null {
  let rest = line;
  const tags: Record<string, string> = {};
  if (rest.startsWith("@")) {
    const sp = rest.indexOf(" ");
    for (const kv of rest.slice(1, sp).split(";")) {
      const eq = kv.indexOf("=");
      if (eq < 0) tags[kv] = "";
      else tags[kv.slice(0, eq)] = unescapeTag(kv.slice(eq + 1));
    }
    rest = rest.slice(sp + 1);
  }
  let prefix: string | null = null;
  if (rest.startsWith(":")) {
    const sp = rest.indexOf(" ");
    prefix = rest.slice(1, sp);
    rest = rest.slice(sp + 1);
  }
  const params: string[] = [];
  while (rest.length) {
    if (rest.startsWith(":")) {
      params.push(rest.slice(1));
      break;
    }
    const sp = rest.indexOf(" ");
    if (sp < 0) {
      params.push(rest);
      break;
    }
    params.push(rest.slice(0, sp));
    rest = rest.slice(sp + 1);
  }
  const command = params.shift();
  return command ? { tags, prefix, command, params } : null;
}

/** "25:0-4,12-16/1902:6-10" -> emotes. */
export function parseEmotes(tag: string | undefined): ChatEmote[] {
  if (!tag) return [];
  const out: ChatEmote[] = [];
  for (const part of tag.split("/")) {
    const [id, ranges] = part.split(":");
    if (!id || !ranges) continue;
    for (const r of ranges.split(",")) {
      const [a, b] = r.split("-").map(Number);
      if (Number.isFinite(a) && Number.isFinite(b)) out.push({ id, start: a, end: b });
    }
  }
  return out.sort((x, y) => x.start - y.start);
}

/** A PRIVMSG as a chat message. */
export function toMessage(l: IrcLine, now = Date.now()): ChatMessage | null {
  if (l.command !== "PRIVMSG") return null;
  const login = (l.prefix ?? "").split("!")[0];
  let text = l.params[1] ?? "";
  let action = false;
  // "/me" arrives wrapped in CTCP: \x01ACTION …\x01.
  const CTCP = String.fromCharCode(1);
  if (text.startsWith(`${CTCP}ACTION `) && text.endsWith(CTCP)) {
    text = text.slice(8, -1);
    action = true;
  }
  return {
    id: l.tags["id"] || `${login}-${now}-${Math.random()}`,
    login,
    name: l.tags["display-name"] || login,
    color: l.tags["color"] || null,
    text,
    emotes: parseEmotes(l.tags["emotes"]),
    action,
    at: now,
  };
}

/** Connects and keeps reconnecting until the returned function is called. */
export function connectChat(channel: string, on: ChatHandlers): () => void {
  let ws: WebSocket | null = null;
  let closed = false;
  let retry = 0;
  let timer: number | undefined;

  const open = () => {
    on.status(retry ? "reconnecting" : "connecting");
    ws = new WebSocket(URL);
    ws.onopen = () => {
      ws!.send("CAP REQ :twitch.tv/tags twitch.tv/commands");
      ws!.send("PASS SCHMOOPIIE");
      ws!.send(`NICK justinfan${10000 + Math.floor(Math.random() * 80000)}`);
      ws!.send(`JOIN #${channel}`);
    };
    ws.onmessage = (e) => {
      for (const raw of String(e.data).split("\r\n")) {
        if (!raw) continue;
        const l = parseLine(raw);
        if (!l) continue;
        switch (l.command) {
          case "PING":
            ws?.send(`PONG :${l.params[0] ?? "tmi.twitch.tv"}`);
            break;
          case "JOIN":
          case "ROOMSTATE":
            retry = 0;
            on.status("connected");
            break;
          case "PRIVMSG": {
            const m = toMessage(l);
            if (m) on.message(m);
            break;
          }
          case "CLEARMSG":
            if (l.tags["target-msg-id"]) on.deleted(l.tags["target-msg-id"]);
            break;
          case "CLEARCHAT":
            on.cleared(l.params[1]?.toLowerCase() ?? null);
            break;
          case "RECONNECT":
            ws?.close();
            break;
        }
      }
    };
    ws.onerror = () => on.status("error");
    ws.onclose = () => {
      ws = null;
      if (closed) return;
      // Back off: 1 s, 2 s, 4 s… up to 30 s.
      const wait = Math.min(30_000, 1000 * 2 ** retry++);
      on.status("reconnecting");
      timer = window.setTimeout(open, wait);
    };
  };
  open();

  return () => {
    closed = true;
    window.clearTimeout(timer);
    ws?.close();
  };
}
