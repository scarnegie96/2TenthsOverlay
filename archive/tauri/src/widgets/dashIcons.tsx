// Dashboard warning-light icons. Simple line drawings on a 32x32 grid,
// coloured by CSS (stroke/fill = currentColor) so they can light up.

import type { ReactNode } from "react";

function Icon({ children }: { children: ReactNode }) {
  return (
    <svg viewBox="0 0 32 32" className="dash-icon" aria-hidden fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      {children}
    </svg>
  );
}

/** Oil can with a drip: oil pressure. */
export const OilPressureIcon = () => (
  <Icon>
    <path d="M5 13h4l3-2h8l7-3-6 10H7a2 2 0 0 1-2-2z" />
    <path d="M14 11V8m-2 0h4" />
    <path d="M28 18c0 1.6-1 2.6-2 2.6s-2-1-2-2.6 2-3.4 2-3.4 2 1.8 2 3.4z" fill="currentColor" stroke="none" />
    <path d="M4 24h22" />
  </Icon>
);

/** Oil drop with a thermometer: oil temperature. */
export const OilTempIcon = () => (
  <Icon>
    <path d="M11 5s-6 7-6 11a6 6 0 0 0 12 0c0-4-6-11-6-11z" />
    <path d="M23 6v13" />
    <circle cx="23" cy="22" r="3" />
    <path d="M26 9h2M26 13h2M26 17h2" />
  </Icon>
);

/** Thermometer in waves: coolant temperature. */
export const WaterTempIcon = () => (
  <Icon>
    <path d="M16 4v13" />
    <circle cx="16" cy="20" r="3" />
    <path d="M19 7h3M19 11h3M19 15h3" />
    <path d="M4 26c2-1.5 4-1.5 6 0s4 1.5 6 0 4-1.5 6 0 4 1.5 6 0" />
  </Icon>
);

/** Fuel pump with a pressure dial: fuel pressure. */
export const FuelPressureIcon = () => (
  <Icon>
    <rect x="5" y="6" width="12" height="21" rx="2" />
    <path d="M17 12h3l3 3v8a2 2 0 0 0 4 0V10l-3-3" />
    <circle cx="11" cy="13" r="4" />
    <path d="M11 13l2-2" />
    <path d="M3 27h16" />
  </Icon>
);

/** Fuel pump: low fuel. */
export const LowFuelIcon = () => (
  <Icon>
    <rect x="5" y="6" width="12" height="21" rx="2" />
    <rect x="8" y="9" width="6" height="5" rx="1" />
    <path d="M17 12h3l3 3v8a2 2 0 0 0 4 0V10l-3-3" />
    <path d="M3 27h16" />
  </Icon>
);

/** Engine block: engine stalled. */
export const StalledIcon = () => (
  <Icon>
    <path d="M7 12h3v-3h9v3h3l3 3h2v8h-2l-3 3H10l-3-3H4v-8h3z" />
    <path d="M13 6h5" />
    <path d="M15.5 15v5" />
    <path d="M13 17.5h5" />
  </Icon>
);

/** P inside a speed dial: pit limiter. */
export const PitLimiterIcon = () => (
  <Icon>
    <path d="M6 22a11 11 0 1 1 20 0" />
    <path d="M13 22V11h4a3 3 0 0 1 0 6h-4" />
    <path d="M4 26h24" />
  </Icon>
);

/** Rev counter pegged at the top: rev limiter. */
export const RevLimiterIcon = () => (
  <Icon>
    <path d="M5 22a11 11 0 1 1 22 0" />
    <path d="M16 22l7-8" />
    <circle cx="16" cy="22" r="2" fill="currentColor" />
    <path d="M23 10l2-2M27 15h2" />
  </Icon>
);

/** Spanner: repairs. */
export const RepairIcon = () => (
  <Icon>
    <path d="M20 5a6 6 0 0 0-5.6 8.2L5 22.6a2.6 2.6 0 0 0 3.7 3.7l9.4-9.4A6 6 0 0 0 26 11.4l-3.6 3.6-4-1.4-1.4-4 3.6-3.6A6 6 0 0 0 20 5z" />
  </Icon>
);

/** Spanner with a plus: optional repairs. */
export const OptionalRepairIcon = () => (
  <Icon>
    <path d="M17 7a5 5 0 0 0-4.7 6.8L5 21.1a2.2 2.2 0 0 0 3.1 3.1l7.3-7.3A5 5 0 0 0 22 12.2l-3 3-3.3-1.2-1.2-3.3 3-3A5 5 0 0 0 17 7z" />
    <path d="M25 20v8M21 24h8" />
  </Icon>
);
