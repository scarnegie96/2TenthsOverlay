// Experimental: the widgets as a web page for an OBS Browser Source, served by
// the app on 127.0.0.1 (src-tauri/src/stream.rs) with live data as
// Server-Sent Events. Transparent, read-only, and never takes the mouse.
//
//   stream.html?widget=relative          one widget, at the top left
//   stream.html?widget=relative&scale=1.5
//   stream.html?layout                   every widget on your main display, where it is on screen
//   stream.html?layout&display=2         …or on display 2

import { createRoot } from "react-dom/client";
import "@fontsource/barlow-semi-condensed/500.css";
import "@fontsource/barlow-semi-condensed/600.css";
import "@fontsource/barlow-semi-condensed/700.css";
import "@fontsource/titillium-web/400.css";
import "@fontsource/titillium-web/600.css";
import "@fontsource/titillium-web/700.css";
import "@fontsource/titillium-web/900.css";
import "../styles/base.css";
import "../styles/overlay.css";
import { connect } from "../lib/store";
import { Stream } from "./Stream";

createRoot(document.getElementById("root")!).render(<Stream />);
void connect({ telemetry: true });
