import { defineConfig } from "vite-plus";

export default defineConfig({
  pack: {
    entry: {
      lint: "src/lint.ts",
      fmt: "src/fmt.ts",
      test: "src/test.ts",
      "preset-textlint": "src/preset-textlint.ts",
    },
    dts: true,
  },
  run: {
    tasks: {
      build: {
        command: "vp pack",
      },
    },
  },
});
