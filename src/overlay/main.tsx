import { createRoot } from "react-dom/client";
import "@fontsource/barlow-semi-condensed/500.css";
import "@fontsource/barlow-semi-condensed/600.css";
import "@fontsource/barlow-semi-condensed/700.css";
import "../styles/base.css";
import "../styles/overlay.css";
import { api, isTauri } from "../lib/bridge";
import { connect } from "../lib/store";
import { Overlay } from "./Overlay";

createRoot(document.getElementById("root")!).render(<Overlay />);

// The overlay window is always loaded, so it applies saved app-level settings on start.
void connect({ telemetry: true }).then((config) => {
  if (isTauri) void api.setDemo(config.general.demo);
});
