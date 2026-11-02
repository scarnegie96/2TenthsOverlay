// Checks the Twitch chat parser on real-shaped IRC lines. Run it like check-config.ts:
//
//   npx tsc --outDir node_modules/.cache/check --module commonjs --moduleResolution node --target ES2021 --strict --skipLibCheck scripts/check-twitch.ts
//   echo '{"type":"commonjs"}' > node_modules/.cache/check/package.json
//   node node_modules/.cache/check/scripts/check-twitch.js

import { channelName, parseEmotes, parseLine, toMessage } from "../src/lib/twitch";

let count = 0;
function check(ok: boolean, what: string) {
  count++;
  if (!ok) throw new Error(`twitch check failed: ${what}`);
}

const line =
  "@badge-info=;badges=moderator/1;color=#1E90FF;display-name=Pit\\sWall;emotes=25:0-4,12-16/1902:6-10;id=abc-123;mod=1 " +
  ":pitwall!pitwall@pitwall.tmi.twitch.tv PRIVMSG #tenths :Kappa Keepo Kappa";
const l = parseLine(line)!;
check(l.command === "PRIVMSG" && l.params[0] === "#tenths", "command and channel");
check(l.tags["display-name"] === "Pit Wall", "escaped tag value");
const m = toMessage(l, 1000)!;
check(m.id === "abc-123" && m.login === "pitwall" && m.name === "Pit Wall" && m.color === "#1E90FF", "message fields");
check(m.text === "Kappa Keepo Kappa" && !m.action, "text");
check(JSON.stringify(m.emotes.map((e) => [e.id, e.start, e.end])) === '[["25",0,4],["1902",6,10],["25",12,16]]', "emotes in order");

const me = toMessage(parseLine(":a!a@a.tmi.twitch.tv PRIVMSG #c :\u0001ACTION waves\u0001")!)!;
check(me.action && me.text === "waves", "/me");
check(parseLine("PING :tmi.twitch.tv")!.command === "PING", "ping");
const clear = parseLine("@room-id=1 :tmi.twitch.tv CLEARCHAT #tenths :spammer")!;
check(clear.command === "CLEARCHAT" && clear.params[1] === "spammer", "timeout");
check(parseEmotes(undefined).length === 0 && parseEmotes("").length === 0, "no emotes");

check(channelName("Tenths_Racing") === "tenths_racing", "plain name");
check(channelName("https://www.twitch.tv/SimSarah/videos") === "simsarah", "link");
check(channelName("#abc") === "abc", "hash");
check(channelName("no spaces allowed") === null && channelName("ab") === null, "invalid");
console.log(`twitch checks passed (${count})`);
