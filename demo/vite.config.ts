import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  // Relative asset paths, so the build works both locally and under the
  // /rusterd/ prefix GitHub Pages serves a project site from.
  base: "./",
  plugins: [solid()],
});
