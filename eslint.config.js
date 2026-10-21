import js from "@eslint/js";
import vue from "eslint-plugin-vue";
import vueTsEslintConfig from "@vue/eslint-config-typescript";
import prettier from "eslint-config-prettier";

export default [
  {
    ignores: ["dist/**", "node_modules/**", "src-tauri/**", "docs/**", ".playwright-mcp/**"],
  },
  js.configs.recommended,
  ...vue.configs["flat/essential"],
  ...vueTsEslintConfig(),
  prettier,
  {
    files: ["**/*.{ts,vue}"],
    rules: {
      "vue/multi-word-component-names": "off",
      "@typescript-eslint/no-explicit-any": "off",
    },
  },
];
