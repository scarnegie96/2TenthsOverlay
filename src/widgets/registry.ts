import type { ComponentType } from "react";
import type { WidgetConfig, WidgetId } from "../lib/config";
import { Coach } from "./Coach";
import { Delta } from "./Delta";
import { Dynamics } from "./Dynamics";
import { Flags } from "./Flags";
import { Fuel } from "./Fuel";
import { Inputs } from "./Inputs";
import { Instruments } from "./Instruments";
import { LapLog } from "./LapLog";
import { Radar } from "./Radar";
import { Relative } from "./Relative";
import { Session } from "./Session";
import { Spotter } from "./Spotter";
import { Standings } from "./Standings";
import { Tires } from "./Tires";

export interface WidgetProps {
  config: WidgetConfig;
  editing: boolean;
}

/** To add a widget: build the component, add its metadata in lib/config.ts, register it here. */
export const COMPONENTS: Record<WidgetId, ComponentType<WidgetProps>> = {
  relative: Relative,
  standings: Standings,
  session: Session,
  inputs: Inputs,
  delta: Delta,
  laplog: LapLog,
  flags: Flags,
  fuel: Fuel,
  spotter: Spotter,
  radar: Radar,
  tires: Tires,
  coach: Coach,
  dynamics: Dynamics,
  instruments: Instruments,
};
