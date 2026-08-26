import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  // Three pages, each its own entry: plain links between them, no router, and
  // nothing that depends on the path the site is served from.
  build: {
    rollupOptions: {
      input: {
        index: "index.html",
        language: "language.html",
        examples: "examples.html",
        demo: "demo.html",
      },
    },
  },
  // Relative asset paths, so the build works both locally and under the
  // /rusterd/ prefix GitHub Pages serves a project site from.
  base: "./",
  plugins: [solid()],
  // The wasm package, the markdown documents and the examples are all read
  // straight out of the repository, so the dev server has to be allowed to
  // serve files from its root.
  server: { fs: { allow: [".."] } },
});
