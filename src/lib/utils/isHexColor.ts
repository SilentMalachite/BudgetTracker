const HEX_COLOR = /^#[0-9A-Fa-f]{6}$/;

/** True only for a strict `#RRGGBB` string, i.e. a value safe to interpolate into an inline style. */
export function isHexColor(value: unknown): value is string {
  return typeof value === 'string' && HEX_COLOR.test(value);
}
