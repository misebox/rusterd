import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  // Relative asset paths, so the build works both locally and under the
  // /rusterd/ prefix GitHub Pages serves a project site from.
  base: "./",
  plugins: [solid()],
  // The wasm package is read straight from ../pkg, so the dev server has to be
  // allowed to serve files from the repository root.
  server: { fs: { allow: [".."] } },
});
