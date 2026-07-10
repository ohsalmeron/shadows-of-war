import { defineConfig } from 'astro/config';

export default defineConfig({
  outDir: './site',
  publicDir: './public',
  base: '/',
  telemetry: false,
});
