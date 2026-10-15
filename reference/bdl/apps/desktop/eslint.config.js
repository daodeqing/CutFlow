import eslint from '@eslint/js'
import vue from 'eslint-plugin-vue'
import globals from 'globals'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  {
    ignores: ['dist/**', 'auto-imports.d.ts', 'components.d.ts'],
  },
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  ...vue.configs['flat/recommended'],
  {
    files: ['**/*.{ts,vue}'],
    languageOptions: {
      globals: globals.browser,
      parserOptions: {
        parser: tseslint.parser,
        extraFileExtensions: ['.vue'],
      },
    },
    rules: {
      'vue/attributes-order': 'off',
      'vue/html-closing-bracket-newline': 'off',
      'vue/html-indent': 'off',
      'vue/html-self-closing': 'off',
      'vue/max-attributes-per-line': 'off',
      'vue/multiline-html-element-content-newline': 'off',
      'vue/multi-word-component-names': 'off',
      'vue/no-v-html': 'off',
      'vue/require-default-prop': 'off',
      'vue/singleline-html-element-content-newline': 'off',
    },
  },
  {
    files: ['**/*.spec.ts'],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
  },
  {
    files: ['src/shared/**/*.{ts,vue}'],
    rules: {
      'no-restricted-imports': ['error', {
        patterns: [{
          group: ['**/desktop/**', '**/mobile/**', '**/app/**'],
          message: 'Shared modules must not import platform views or application orchestration.',
        }],
      }],
    },
  },
  {
    files: ['src/desktop/**/*.{ts,vue}'],
    rules: {
      'no-restricted-imports': ['error', {
        patterns: [{
          group: ['**/mobile/**', '**/app/AppWorkspace*'],
          message: 'Desktop views depend on shared modules, not mobile views or application orchestration.',
        }],
      }],
    },
  },
  {
    files: ['src/mobile/**/*.{ts,vue}'],
    rules: {
      'no-restricted-imports': ['error', {
        patterns: [{
          group: ['**/desktop/**', '**/app/AppWorkspace*'],
          message: 'Mobile views depend on shared modules, not desktop views or application orchestration.',
        }],
      }],
    },
  },
)
