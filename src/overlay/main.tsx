import { createRoot } from "react-dom/client";
import "@fontsource/barlow-semi-condensed/500.css";
import "@fontsource/barlow-semi-condensed/600.css";
import "@fontsource/barlow-semi-condensed/700.css";
import "../styles/base.css";
import "../styles/overlay.css";
import { api, isTauri } from "../lib/bridge";
import { connect } from "../lib/store";
import type { WidgetId } from "../lib/config";
import { Overlay } from "./Overlay";
import { Shot } from "./Shot";

const params = new URLSearchParams(location.search);
// Browser preview: ?shot=<widget> for README screenshots, ?theme=light, ?clean hides the preview hint.
const shot = !isTauri ? (params.get("shot") as WidgetId | null) : null;
if (!isTauri && params.has("clean")) document.body.classList.add("is-clean");

createRoot(document.getElementById("root")!).render(shot ? <Shot id={shot} /> : <Overlay />);

// The overlay window is always loaded, so it applies saved app-level settings on start.
void connect({ telemetry: true }).then((config) => {
  if (isTauri) void api.setDemo(config.general.demo);
});
