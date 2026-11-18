#version 300 es
// Ray marches light from the camera through Schwarzschild spacetime.
// Units are G = c = M = 1: horizon at r = 2, ISCO at r = 6.
precision highp float;

uniform vec2 uSize;
uniform float uCellAspect;
uniform float uTime;
uniform vec3 uCamPos;
uniform mat3 uCamRot;

out vec4 outColor;

const float PI = 3.14159265;
const float HORIZON = 2.0;
const float DISK_IN = 6.0;
const float DISK_OUT = 13.0;

float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float hash(vec3 p) {
  p = fract(p * vec3(0.1031, 0.1030, 0.0973));
  p += dot(p, p.yxz + 33.33);
  return fract((p.x + p.y) * p.z);
}

// value noise, x wraps every `period` cells
float noise(vec2 p, float period) {
  vec2 i = floor(p);
  vec2 f = smoothstep(0.0, 1.0, fract(p));
  float x0 = mod(i.x, period);
  float x1 = mod(i.x + 1.0, period);
  return mix(mix(hash(vec2(x0, i.y)), hash(vec2(x1, i.y)), f.x),
             mix(hash(vec2(x0, i.y + 1.0)), hash(vec2(x1, i.y + 1.0)), f.x), f.y);
}

float noise(vec3 p) {
  vec3 i = floor(p);
  vec3 f = smoothstep(0.0, 1.0, fract(p));
  vec2 e = vec2(1.0, 0.0);
  return mix(mix(mix(hash(i), hash(i + e.xyy), f.x),
                 mix(hash(i + e.yxy), hash(i + e.xxy), f.x), f.y),
             mix(mix(hash(i + e.yyx), hash(i + e.xyx), f.x),
                 mix(hash(i + e.yxx), hash(i + e.xxx), f.x), f.y), f.z);
}

// g < 1 redshifted, g > 1 blueshifted
vec3 gasColor(float g) {
  vec3 c = mix(vec3(0.45, 0.05, 0.02), vec3(1.0, 0.35, 0.08), smoothstep(0.3, 0.75, g));
  c = mix(c, vec3(1.0, 0.72, 0.38), smoothstep(0.7, 1.05, g));
  c = mix(c, vec3(1.0, 0.95, 0.85), smoothstep(1.0, 1.35, g));
  return mix(c, vec3(0.7, 0.82, 1.0), smoothstep(1.35, 1.8, g));
}

vec4 disk(vec3 p, vec3 dir) {
  float r = length(p.xz);
  if (r < DISK_IN || r > DISK_OUT) return vec4(0.0);

  // keplerian rotation, so the streaks shear over time
  float a = (atan(p.z, p.x) - uTime * 9.0 * pow(r, -1.5)) / (2.0 * PI);
  float gas = 0.5 * noise(vec2(a * 16.0, r * 1.6), 16.0)
            + 0.3 * noise(vec2(a * 40.0, r * 4.0), 40.0)
            + 0.2 * noise(vec2(a * 90.0, r * 9.0), 90.0);
  gas = smoothstep(0.2, 0.9, gas);

  // doppler + gravitational redshift
  float v = min(0.99, sqrt(1.0 / (r - 2.0)));
  float gamma = inversesqrt(1.0 - v * v);
  float cosTheta = dot(normalize(vec3(-p.z, 0.0, p.x)), -normalize(dir));
  float g = sqrt(1.0 - 2.0 / r) / (gamma * (1.0 - v * cosTheta));

  float edge = smoothstep(DISK_IN, DISK_IN + 0.6, r) * (1.0 - smoothstep(DISK_OUT - 5.0, DISK_OUT, r));
  float light = pow(DISK_IN / r, 2.0) * (0.25 + 1.6 * gas) * pow(g, 3.0) * 2.6;
  return vec4(gasColor(g) * light, edge * (0.75 + 0.25 * gas));
}

vec3 sky(vec3 d) {
  vec3 c = vec3(0.0);

  float h = hash(floor(d * 140.0));
  if (h > 0.992) {
    c += vec3(0.85, 0.9, 1.0) * (h - 0.992) * 125.0 * (0.7 + 0.3 * sin(uTime * 3.0 + h * 900.0));
  }

  float band = exp(-pow(dot(d, normalize(vec3(0.3, 1.0, 0.45))), 2.0) * 6.0);
  float dust = noise(d * 4.0) * 0.6 + noise(d * 11.0) * 0.4;
  vec3 tint = mix(vec3(0.3, 0.22, 0.6), vec3(0.15, 0.25, 0.65), noise(d * 2.0 + 7.0));
  return c + tint * smoothstep(0.35, 0.85, dust) * (0.22 + 0.4 * band);
}

void main() {
  vec2 uv = gl_FragCoord.xy / uSize * 2.0 - 1.0;
  float aspect = uSize.y * uCellAspect / uSize.x;
  uv.y *= aspect;

  // zoom in on tall (phone) screens so the disk still fills the width
  float zoom = 0.62 / clamp(aspect / 0.65, 1.0, 1.5);

  vec3 pos = uCamPos;
  vec3 dir = normalize(uCamRot * vec3(uv * zoom, 1.0));

  // null geodesic as a newtonian orbit with an extra -1.5 h^2 / r^5 term
  vec3 h = cross(pos, dir);
  float h2 = dot(h, h);
  float escape = length(uCamPos) * 1.3;

  vec3 color = vec3(0.0);
  float transmittance = 1.0;
  bool escaped = false;

  for (int i = 0; i < 400; i++) {
    float r = length(pos);
    if (r < HORIZON) break;
    if (r > escape && dot(pos, dir) > 0.0) {
      escaped = true;
      break;
    }

    float dt = min(1.5, 0.04 * r * max(1.0, r - 2.5));
    dir -= 1.5 * h2 * pos / pow(r, 5.0) * dt;
    vec3 next = pos + dir * dt;

    if (pos.y * next.y < 0.0) {
      vec4 gas = disk(mix(pos, next, pos.y / (pos.y - next.y)), dir);
      color += transmittance * gas.a * gas.rgb;
      transmittance *= 1.0 - gas.a;
      if (transmittance < 0.02) break;
    }
    pos = next;
  }
  if (escaped) color += transmittance * sky(normalize(dir));

  outColor = vec4(1.0 - exp(-color * 1.3), 1.0);
}
