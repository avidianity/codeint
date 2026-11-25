# codeint report

codeint measured 1 file with 7 source lines.

Test gap is not measured because no Coverage report was named.

| Source set | Metric | Value | Behind the value | Reference range |
| ---------- | ------ | ----- | ---------------- | --------------- |
| Production | Erosion | 0 | 0 of 5.2915 Function mass in eroded functions | 0.352 to 0.6, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Cognitive complexity | 0 | 0 of 2 cognitive complexity in functions over 15 | no published Reference range for this language |
| Production | Duplication | 0 | 0 clone lines of 7 source lines | 0.017 to 0.346, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Verbosity | 0.1429 | 1 Flagged line of 7 source lines | 0.018 to 0.347, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Import cycles | 0 | 0 of 1 module in an Import cycle | 0 to 0.308, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |

## Production Findings

### Verbosity

| Flagged line | Rule |
| ------------ | ---- |
| `src/sign.ts:4` | else-after-return |

## Files not measured

- `src/broken.ts`: cannot parse: Unexpected token at line 1, column 14
- `src/types.d.ts`: declaration only
