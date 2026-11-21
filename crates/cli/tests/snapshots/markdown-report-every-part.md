# codeint report

codeint measured 28 files with 516 source lines: Production has 27 files with 514 source lines, Test has 1 file with 2 source lines.

| Source set | Metric | Value | Behind the value | Reference range |
| ---------- | ------ | ----- | ---------------- | --------------- |
| Production | Erosion | 0.9894 | 2177.4789 of 2200.7704 Function mass in eroded functions | 0.352 to 0.6, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Duplication | 0.035 | 18 clone lines of 514 source lines | 0.017 to 0.346, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Verbosity | 0.037 | 19 verbose lines of 514 source lines: 1 Flagged line, 18 clone lines | 0.018 to 0.347, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Import cycles | 0.0741 | 2 of 27 modules in an Import cycle, 1 unresolved import | 0 to 0.308, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Test | Erosion | 0 | 0 of 0 Function mass in eroded functions | 0.352 to 0.6, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Test | Duplication | 0 | 0 clone lines of 2 source lines | 0.017 to 0.346, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Test | Verbosity | 0 | 0 verbose lines of 2 source lines: 0 Flagged lines, 0 clone lines | 0.018 to 0.347, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Test | Import cycles | 0 | 0 of 1 module in an Import cycle | 0 to 0.308, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |

## Production Findings

### Erosion

| Function | Cyclomatic complexity | Source lines | Function mass |
| -------- | --------------------- | ------------ | ------------- |
| `src/f21.ts:1` | 31 | 33 | 178.0814 |
| `src/f20.ts:1` | 30 | 32 | 169.7056 |
| `src/f19.ts:1` | 29 | 31 | 161.4652 |
| `src/f18.ts:1` | 28 | 30 | 153.3623 |
| `src/f17.ts:1` | 27 | 29 | 145.3994 |
| `src/f16.ts:1` | 26 | 28 | 137.5791 |
| `src/f15.ts:1` | 25 | 27 | 129.9038 |
| `src/f14.ts:1` | 24 | 26 | 122.3765 |
| `src/f13.ts:1` | 23 | 25 | 115 |
| `src/f12.ts:1` | 22 | 24 | 107.7775 |
| `src/f11.ts:1` | 21 | 23 | 100.7125 |
| `src/f10.ts:1` | 20 | 22 | 93.8083 |
| `src/f09.ts:1` | 19 | 21 | 87.0689 |
| `src/f08.ts:1` | 18 | 20 | 80.4984 |
| `src/f07.ts:1` | 17 | 19 | 74.1013 |
| `src/f06.ts:1` | 16 | 18 | 67.8823 |
| `src/f05.ts:1` | 15 | 17 | 61.8466 |
| `src/f04.ts:1` | 14 | 16 | 56 |
| `src/f03.ts:1` | 13 | 15 | 50.3488 |
| `src/f02.ts:1` | 12 | 14 | 44.8999 |

1 more eroded function not listed. Pass `--all` to list every one.

### Duplication

- 83 tokens: `src/cart.ts:1-9`, `src/order.ts:1-9`

### Verbosity

| Flagged line | Rule |
| ------------ | ---- |
| `src/sign.ts:4` | else-after-return |

### Import cycles

- `src/a.ts`, `src/b.ts`

### Unresolved imports

| Importer | Specifier | Reason |
| -------- | --------- | ------ |
| `src/pipe.ts` | ``./a\|b`c`` | no file of the repository matches "./a\|b\`c" |

## Files not measured

- `src/broken.ts`: cannot parse: Unexpected token at line 1, column 14
- `src/types.d.ts`: declaration only
