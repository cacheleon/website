#version 300 es
// Turns the scene texture (2x2 texels per cell) into coloured glyphs.
precision highp float;

uniform sampler2D uScene;
uniform sampler2D uGlyphs;
uniform vec2 uCanvas;
uniform vec2 uCell;
uniform float uGlyphCount;

out vec4 outColor;

void main() {
  vec2 p = vec2(gl_FragCoord.x, uCanvas.y - gl_FragCoord.y);
  vec2 cell = floor(p / uCell);
  vec2 cells = floor(uCanvas / uCell);

  vec2 uv = vec2((cell.x + 0.5) / cells.x, 1.0 - (cell.y + 0.5) / cells.y);
  vec3 c = textureLod(uScene, uv, 0.0).rgb;
  float lum = max(c.r, max(c.g, c.b));

  float glyph = lum < 0.07 ? 0.0 : max(1.0, floor(min((lum - 0.07) / 0.85, 0.999) * uGlyphCount));
  vec2 glyphUv = vec2((glyph + fract(p.x / uCell.x)) / uGlyphCount, fract(p.y / uCell.y));
  float ink = texture(uGlyphs, glyphUv).r;
  vec3 tint = c / max(lum, 0.001) * (0.45 + 0.75 * lum);

  vec2 screenUv = vec2(p.x / uCanvas.x, 1.0 - p.y / uCanvas.y);
  vec3 bloom = textureLod(uScene, screenUv, 2.0).rgb * 0.1
             + textureLod(uScene, screenUv, 4.0).rgb * 0.22;

  outColor = vec4(tint * ink + bloom, 1.0);
}
