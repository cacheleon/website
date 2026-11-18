import vertSrc from "./fullscreen.vert?raw";
import sceneSrc from "./scene.frag?raw";
import asciiSrc from "./ascii.frag?raw";

const GLYPHS = " .`:-~=+*xo%#&@";
const FONT = "ui-monospace, SFMono-Regular, Menlo, monospace";

type Vec3 = [number, number, number];

function cross(a: Vec3, b: Vec3): Vec3 {
  return [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
}

function normalize(v: Vec3): Vec3 {
  const len = Math.hypot(...v);
  return [v[0] / len, v[1] / len, v[2] / len];
}

function program(gl: WebGL2RenderingContext, frag: string, uniforms: string[]) {
  const prog = gl.createProgram();
  for (const [type, src] of [
    [gl.VERTEX_SHADER, vertSrc],
    [gl.FRAGMENT_SHADER, frag],
  ] as const) {
    const shader = gl.createShader(type)!;
    gl.shaderSource(shader, src);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      throw new Error(gl.getShaderInfoLog(shader) ?? "");
    }
    gl.attachShader(prog, shader);
  }
  gl.bindAttribLocation(prog, 0, "aPos");
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    throw new Error(gl.getProgramInfoLog(prog) ?? "");
  }

  const loc: Record<string, WebGLUniformLocation | null> = {};
  for (const name of uniforms) loc[name] = gl.getUniformLocation(prog, name);
  return { prog, loc };
}

function texture(gl: WebGL2RenderingContext, filter: number) {
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
  gl.texParameteri(
    gl.TEXTURE_2D,
    gl.TEXTURE_MAG_FILTER,
    filter === gl.NEAREST ? gl.NEAREST : gl.LINEAR,
  );
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  return tex;
}

function glyphAtlas(cellW: number, cellH: number, fontPx: number) {
  const canvas = document.createElement("canvas");
  canvas.width = cellW * GLYPHS.length;
  canvas.height = cellH;

  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#000";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#fff";
  ctx.font = `${fontPx}px ${FONT}`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  for (let i = 0; i < GLYPHS.length; i++) {
    ctx.fillText(GLYPHS[i], (i + 0.5) * cellW, cellH / 2);
  }
  return canvas;
}

// Fly in for 7s, then orbit and swing up and down. All the post-intro
// terms start with zero velocity so there's no jerk at the hand-off.
function camera(t: number) {
  const intro = 1 - (1 - Math.min(1, t / 7)) ** 3;
  const s = Math.max(0, t - 7);

  const dist = 95 - 65 * intro + 5 * Math.sin(s * 0.07) * Math.min(1, s / 10);
  const tilt = 0.5 - 0.32 * intro + 0.45 * (1 - Math.cos(s * 0.11));
  const yaw = 0.6 * (1 - intro) + 0.1 * (s - 4 * (1 - Math.exp(-s / 4)));
  const roll = 0.12 * Math.sin(s * 0.05);

  const pos: Vec3 = [
    dist * Math.cos(tilt) * Math.sin(yaw),
    dist * Math.sin(tilt),
    -dist * Math.cos(tilt) * Math.cos(yaw),
  ];
  const fwd = normalize([-pos[0], -pos[1], -pos[2]]);
  const r0 = normalize(cross([0, 1, 0], fwd));
  const u0 = cross(fwd, r0);

  const c = Math.cos(roll);
  const sn = Math.sin(roll);
  const right = r0.map((v, i) => c * v + sn * u0[i]);
  const up = u0.map((v, i) => c * v - sn * r0[i]);

  return { pos, rot: [...right, ...up, ...fwd] };
}

export function startBlackHole(canvas: HTMLCanvasElement, still: boolean) {
  const gl = canvas.getContext("webgl2", { antialias: false, alpha: false });
  if (!gl) return false;

  const box = canvas.parentElement!;
  const scene = program(gl, sceneSrc, [
    "uSize",
    "uCellAspect",
    "uTime",
    "uCamPos",
    "uCamRot",
  ]);
  const ascii = program(gl, asciiSrc, [
    "uScene",
    "uGlyphs",
    "uCanvas",
    "uCell",
    "uGlyphCount",
  ]);

  // fullscreen triangle
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([-1, -1, 3, -1, -1, 3]),
    gl.STATIC_DRAW,
  );
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

  const sceneTex = texture(gl, gl.LINEAR_MIPMAP_LINEAR);
  const glyphTex = texture(gl, gl.NEAREST);
  const fbo = gl.createFramebuffer();

  let cols = 0;
  let rows = 0;
  let cellW = 0;
  let cellH = 0;

  const resize = () => {
    // Size the canvas in device pixels and never let CSS scale it: any
    // resampling makes some glyph columns look bolder than others.
    const dpr = devicePixelRatio || 1;
    const fontPx = Math.round(
      Math.min(12, Math.max(6, box.clientWidth / 190)) * dpr,
    );

    const measure = document.createElement("canvas").getContext("2d")!;
    measure.font = `${fontPx}px ${FONT}`;
    cellW = Math.max(1, Math.round(measure.measureText("M").width));
    cellH = Math.round(fontPx * 1.2);
    cols = Math.floor((box.clientWidth * dpr) / cellW);
    rows = Math.floor((box.clientHeight * dpr) / cellH);

    canvas.width = cols * cellW;
    canvas.height = rows * cellH;
    canvas.style.width = `${canvas.width / dpr}px`;
    canvas.style.height = `${canvas.height / dpr}px`;

    gl.bindTexture(gl.TEXTURE_2D, glyphTex);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      glyphAtlas(cellW, cellH, fontPx),
    );

    gl.bindTexture(gl.TEXTURE_2D, sceneTex);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA8,
      cols * 2,
      rows * 2,
      0,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      null,
    );
    gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
    gl.framebufferTexture2D(
      gl.FRAMEBUFFER,
      gl.COLOR_ATTACHMENT0,
      gl.TEXTURE_2D,
      sceneTex,
      0,
    );
  };

  const draw = (t: number) => {
    const cam = camera(t);

    gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
    gl.viewport(0, 0, cols * 2, rows * 2);
    gl.useProgram(scene.prog);
    gl.uniform2f(scene.loc.uSize, cols * 2, rows * 2);
    gl.uniform1f(scene.loc.uCellAspect, cellH / cellW);
    gl.uniform1f(scene.loc.uTime, t);
    gl.uniform3fv(scene.loc.uCamPos, cam.pos);
    gl.uniformMatrix3fv(scene.loc.uCamRot, false, cam.rot);
    gl.drawArrays(gl.TRIANGLES, 0, 3);

    // mip levels double as the bloom blur
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, sceneTex);
    gl.generateMipmap(gl.TEXTURE_2D);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, glyphTex);

    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.useProgram(ascii.prog);
    gl.uniform1i(ascii.loc.uScene, 0);
    gl.uniform1i(ascii.loc.uGlyphs, 1);
    gl.uniform2f(ascii.loc.uCanvas, canvas.width, canvas.height);
    gl.uniform2f(ascii.loc.uCell, cellW, cellH);
    gl.uniform1f(ascii.loc.uGlyphCount, GLYPHS.length);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.activeTexture(gl.TEXTURE0);
  };

  const start = performance.now();
  let visible = true;
  let running = false;

  function frame() {
    running = visible;
    if (!visible) return;
    draw((performance.now() - start) / 1000);
    requestAnimationFrame(frame);
  }

  function play() {
    if (still) draw(40);
    else if (visible && !running) {
      running = true;
      requestAnimationFrame(frame);
    }
  }

  new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
    play();
  }).observe(canvas);

  new ResizeObserver(() => {
    resize();
    if (still) draw(40);
  }).observe(box);

  resize();
  play();
  return true;
}
