import { defineConfig } from 'eslint/config';
import globals from 'globals';
import eslintConfigPrettier from 'eslint-config-prettier';

export default defineConfig([
    // --- Global ignores -------------------------------------------------------
    {
        ignores: [
            'node_modules/**',
            'dist/**',
            'src-tauri/**',
            'public/javascripts/**', // third-party scripts
        ],
    },

    // --- Main rules block -----------------------------------------------------
    {
        files: ['**/*.js', '**/*.mjs', '**/*.cjs'],

        languageOptions: {
            ecmaVersion: 2022,
            sourceType: 'module',
            globals: {
                ...globals.node, // process, Buffer, __dirname, setTimeout …
                ...globals.nodeBuiltin, // built-in module names
            },
        },

        rules: {
            // -----------------------------------------------------------------
            // ERROR SAFETY
            // -----------------------------------------------------------------
            'no-undef': 'error', // catch missing imports
            'no-unused-vars': [
                'warn',
                {
                    vars: 'all',
                    args: 'after-used',
                    // prefix with _ to intentionally ignore a var/arg
                    varsIgnorePattern: '^_',
                    argsIgnorePattern: '^_',
                },
            ],
            'no-constant-binary-expression': 'error', // e.g. a || true
            'no-unreachable': 'error',
            'no-duplicate-case': 'error',
            'no-empty': ['error', { allowEmptyCatch: false }],
            'no-prototype-builtins': 'error', // obj.hasOwnProperty → Object.prototype.hasOwnProperty
            'no-throw-literal': 'error', // throw new Error() not throw 'string'
            'no-promise-executor-return': 'error', // return inside Promise executor does nothing
            'no-constructor-return': 'error',
            'no-self-compare': 'error',
            'use-isnan': 'error',
            'valid-typeof': ['error', { requireStringLiterals: true }],

            // -----------------------------------------------------------------
            // SECURITY  (OWASP Top 10 mitigations)
            // -----------------------------------------------------------------
            'no-eval': 'error', // OWASP A03 Injection
            'no-new-func': 'error', // same as eval via constructor
            'no-implied-eval': 'error', // setTimeout('code') etc.
            'no-script-url': 'error', // javascript: URLs
            'no-extend-native': 'error', // don't patch built-ins
            'no-global-assign': 'error', // don't overwrite globals

            // -----------------------------------------------------------------
            // CODE QUALITY
            // -----------------------------------------------------------------
            'prefer-const': 'error', // use const unless reassigned
            'no-var': 'error', // ES6+ — no var
            eqeqeq: ['error', 'always', { null: 'ignore' }],
            curly: ['error', 'all'], // always brace if/else bodies
            'no-return-await': 'warn', // async fn: return await is a no-op
            'no-await-in-loop': 'warn', // prefer Promise.all
            'no-lone-blocks': 'warn',
            'no-useless-catch': 'error',
            'no-useless-concat': 'warn',
            'prefer-template': 'warn', // prefer `${x}` over '' + x
            'object-shorthand': 'warn', // { fn: fn } → { fn }
        },
    },

    // --- Prettier last: disables all format rules that conflict ---------------
    // Must be the final entry so it wins over everything above.
    eslintConfigPrettier,
]);
