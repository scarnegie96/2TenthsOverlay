import type { ComponentType } from "react";
import type { WidgetConfig, WidgetId } from "../lib/config";
import { Delta } from "./Delta";
import { Fuel } from "./Fuel";
import { Inputs } from "./Inputs";
import { Relative } from "./Relative";
import { Spotter } from "./Spotter";
import { Standings } from "./Standings";

export interface WidgetProps {
  config: WidgetConfig;
  editing: boolean;
}

/** To add a widget: build the component, add its metadata in lib/config.ts, register it here. */
export const COMPONENTS: Record<WidgetId, ComponentType<WidgetProps>> = {
  relative: Relative,
  standings: Standings,
  inputs: Inputs,
  delta: Delta,
  fuel: Fuel,
  spotter: Spotter,
};
