// Smart numeric formatter for slider value chips.
//
// Picks precision based on magnitude so:
//   - sub-1 values don't truncate to "0" (e.g. 0.25 Hz LFO rate)
//   - mid-range values keep useful detail (e.g. 12.4 Hz)
//   - large values aren't noisy (e.g. 1240 Hz, not 1240.00)
export function smartFormat(v: number, unit?: string): string {
  const abs = Math.abs(v);
  let body: string;
  if (abs === 0) body = '0';
  else if (abs < 10) body = v.toFixed(2);
  else if (abs < 100) body = v.toFixed(1);
  else body = v.toFixed(0);
  return unit ? `${body} ${unit}` : body;
}
