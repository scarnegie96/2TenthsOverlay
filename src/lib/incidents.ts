// The incident count you're working towards. A session's limit (17x, say)
// isn't the end in series that hand out a penalty there and carry on: after
// it, the next penalty comes every few incidents more (21x, 25x…). iRacing's
// session info only gives the first limit, so the step comes from the
// Relative's settings. If iRacing raises the limit it reports, that's used.

export interface IncidentTarget {
  /** The incident count of the next penalty, or null when there's no limit. */
  next: number | null;
  /** Limits already reached this session. */
  passed: number;
}

export function incidentTarget(count: number, limit: number | null, step: number): IncidentTarget {
  if (limit == null || limit <= 0) return { next: null, passed: 0 };
  if (count < limit) return { next: limit, passed: 0 };
  if (step <= 0) return { next: limit, passed: 1 };
  const passed = Math.floor((count - limit) / step) + 1;
  return { next: limit + passed * step, passed };
}
