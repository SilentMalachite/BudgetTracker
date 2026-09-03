import { describe, expect, it } from 'vitest';
import { isHexColor } from './isHexColor';

describe('isHexColor', () => {
  it('accepts #RRGGBB in either case', () => {
    expect(isHexColor('#4FACFE')).toBe(true);
    expect(isHexColor('#ff7a85')).toBe(true);
    expect(isHexColor('#000000')).toBe(true);
  });

  it('rejects anything that is not exactly #RRGGBB', () => {
    expect(isHexColor(null)).toBe(false);
    expect(isHexColor(undefined)).toBe(false);
    expect(isHexColor('')).toBe(false);
    expect(isHexColor('#FFF')).toBe(false);
    expect(isHexColor('#FFFFFFFF')).toBe(false);
    expect(isHexColor('#GGGGGG')).toBe(false);
    expect(isHexColor('FFFFFF')).toBe(false);
    expect(isHexColor('red')).toBe(false);
    expect(isHexColor(' #FFFFFF')).toBe(false);
    expect(isHexColor('#FFFFFF; background-image: url(x)')).toBe(false);
    expect(isHexColor(0xffffff)).toBe(false);
  });
});
