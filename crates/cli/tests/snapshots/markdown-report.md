# codeint report

codeint measured 1 file with 7 source lines.

| Source set | Metric | Value | Behind the value | Reference range |
| ---------- | ------ | ----- | ---------------- | --------------- |
| Production | Erosion | 0 | 0 of 5.2915 Function mass in eroded functions | no published Reference range for this language |
| Production | Duplication | 0 | 0 clone lines of 7 source lines | no published Reference range for this language |
| Production | Verbosity | 0.1429 | 1 verbose line of 7 source lines: 1 Flagged line, 0 clone lines | no published Reference range for this language |
| Production | Import cycles | 0 | 0 of 1 module in an Import cycle | no published Reference range for this language |

## Production Findings

### Verbosity

| Flagged line | Rule |
| ------------ | ---- |
| `src/sign.ts:4` | else-after-return |

## Files not measured

- `src/broken.ts`: cannot parse: Unexpected token at line 1, column 14
- `src/types.d.ts`: declaration only
