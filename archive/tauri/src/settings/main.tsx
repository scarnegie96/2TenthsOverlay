import { createRoot } from "react-dom/client";
import "@fontsource/barlow-semi-condensed/500.css";
import "@fontsource/barlow-semi-condensed/600.css";
import "@fontsource/barlow-semi-condensed/700.css";
import "@fontsource/titillium-web/400.css";
import "@fontsource/titillium-web/600.css";
import "@fontsource/titillium-web/700.css";
import "@fontsource/titillium-web/900.css";
import "../styles/base.css";
import "../styles/settings.css";
import { connect } from "../lib/store";
import { Settings } from "./Settings";

createRoot(document.getElementById("root")!).render(<Settings />);
void connect({ telemetry: false });
