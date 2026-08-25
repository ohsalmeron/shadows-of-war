/**
 * Shadows of War — Tactical Perspective Grid WebGL Canvas
 * Cortiz-inspired procedural shader for real-time war-room command map backdrop.
 * Theme-aware (Dark/Light mode) with zero external dependencies.
 */
(function(global) {
  function initTacticalGrid(canvasId = 'tactical-grid') {
    const canvas = document.getElementById(canvasId);
    if (!canvas) return null;

    const gl = canvas.getContext('webgl', { alpha: true, antialias: true });
    if (!gl) {
      console.warn('[sow-web] WebGL not supported on tactical grid canvas');
      return null;
    }

    const vsSource = `
      attribute vec2 aPos;
      varying vec2 vUv;
      void main() {
        vUv = aPos * 0.5 + 0.5;
        gl_Position = vec4(aPos, 0.0, 1.0);
      }
    `;

    const fsSource = `
      precision highp float;
      varying vec2 vUv;
      uniform float uTime;
      uniform vec2 uResolution;
      uniform vec3 uGridColor;
      uniform vec3 uBgColor;
      uniform float uOpacity;

      void main() {
        // Center-relative coordinates normalized with aspect ratio
        vec2 coord = (vUv - 0.5) * vec2(uResolution.x / max(uResolution.y, 1.0), 1.0);
        
        // War-room perspective projection (horizon tilt)
        float depth = 0.55 / max(coord.y + 0.85, 0.02);
        
        // Animated tactical grid UV coordinates
        vec2 gridUv = vec2(coord.x * depth * 2.2, depth * 1.5 + uTime * 0.035) * 6.0;
        
        // Multi-tier grid lines calculation with derivative anti-aliasing
        vec2 gridDeriv = fwidth(gridUv);
        vec2 gridMajor = abs(fract(gridUv - 0.5) - 0.5) / max(gridDeriv, vec2(0.001));
        float lineMajor = min(gridMajor.x, gridMajor.y);
        float maskMajor = 1.0 - min(lineMajor, 1.0);

        // Minor grid subdivisions
        vec2 gridMinor = abs(fract(gridUv * 2.0 - 0.5) - 0.5) / max(gridDeriv * 2.0, vec2(0.001));
        float lineMinor = min(gridMinor.x, gridMinor.y);
        float maskMinor = (1.0 - min(lineMinor, 1.0)) * 0.45;

        float combinedGrid = max(maskMajor, maskMinor);
        
        // Tactical vignette and horizon falloff
        float horizonFade = smoothstep(-0.4, 0.45, coord.y + 0.4) * smoothstep(3.2, 0.3, depth);
        float radialMask = 1.0 - smoothstep(0.3, 0.95, length(coord * vec2(0.85, 1.2)));

        float finalAlpha = combinedGrid * horizonFade * radialMask * uOpacity;
        
        vec3 finalColor = mix(uBgColor, uGridColor, finalAlpha);
        gl_FragColor = vec4(finalColor, finalAlpha);
      }
    `;

    function compileShader(type, src) {
      const shader = gl.createShader(type);
      gl.shaderSource(shader, src);
      gl.compileShader(shader);
      if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
        console.error('[sow-web] Shader compile error:', gl.getShaderInfoLog(shader));
        gl.deleteShader(shader);
        return null;
      }
      return shader;
    }

    const vertShader = compileShader(gl.VERTEX_SHADER, vsSource);
    const fragShader = compileShader(gl.FRAGMENT_SHADER, fsSource);
    if (!vertShader || !fragShader) return null;

    const program = gl.createProgram();
    gl.attachShader(program, vertShader);
    gl.attachShader(program, fragShader);
    gl.linkProgram(program);

    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      console.error('[sow-web] Program link error:', gl.getProgramInfoLog(program));
      return null;
    }

    gl.useProgram(program);

    // Full-screen quad geometry
    const vertexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vertexBuffer);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]),
      gl.STATIC_DRAW
    );

    const aPosLoc = gl.getAttribLocation(program, 'aPos');
    gl.enableVertexAttribArray(aPosLoc);
    gl.vertexAttribPointer(aPosLoc, 2, gl.FLOAT, false, 0, 0);

    const uTimeLoc = gl.getUniformLocation(program, 'uTime');
    const uResLoc = gl.getUniformLocation(program, 'uResolution');
    const uGridColorLoc = gl.getUniformLocation(program, 'uGridColor');
    const uBgColorLoc = gl.getUniformLocation(program, 'uBgColor');
    const uOpacityLoc = gl.getUniformLocation(program, 'uOpacity');

    let animationFrameId = null;
    let isVisible = true;

    function resize() {
      const parent = canvas.parentElement || document.body;
      const width = parent.clientWidth || window.innerWidth;
      const height = parent.clientHeight || 700;
      const dpr = Math.min(window.devicePixelRatio || 1, 2);

      canvas.width = width * dpr;
      canvas.height = height * dpr;
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;

      gl.viewport(0, 0, canvas.width, canvas.height);
    }

    window.addEventListener('resize', resize, { passive: true });
    resize();

    // IntersectionObserver to pause rendering when off-screen (0% idle CPU)
    if ('IntersectionObserver' in window) {
      const observer = new IntersectionObserver((entries) => {
        isVisible = entries[0].isIntersecting;
      }, { threshold: 0.05 });
      observer.observe(canvas);
    }

    const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

    function render(timestamp) {
      if (isVisible && !prefersReducedMotion.matches) {
        const seconds = timestamp * 0.001;
        gl.uniform1f(uTimeLoc, seconds);
        gl.uniform2f(uResLoc, canvas.width, canvas.height);

        const isLight = document.documentElement.dataset.theme === 'light';
        if (isLight) {
          // Tactical War Orange (#ff7a00) on Parchment (#ede7d8)
          gl.uniform3f(uGridColorLoc, 1.0, 0.48, 0.0);
          gl.uniform3f(uBgColorLoc, 0.93, 0.905, 0.847);
          gl.uniform1f(uOpacityLoc, 0.28);
        } else {
          // Signature War Orange (#ff5500) on Obsidian (#0a0a0e)
          gl.uniform3f(uGridColorLoc, 1.0, 0.333, 0.0);
          gl.uniform3f(uBgColorLoc, 0.039, 0.039, 0.055);
          gl.uniform1f(uOpacityLoc, 0.42);
        }

        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      }
      animationFrameId = requestAnimationFrame(render);
    }

    animationFrameId = requestAnimationFrame(render);

    return {
      destroy: () => {
        if (animationFrameId) cancelAnimationFrame(animationFrameId);
        window.removeEventListener('resize', resize);
      }
    };
  }

  global.initTacticalGrid = initTacticalGrid;
})(typeof window !== 'undefined' ? window : this);
