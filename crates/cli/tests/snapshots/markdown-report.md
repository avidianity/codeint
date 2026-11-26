# codeint report

codeint measured 1 file with 7 source lines.

Test gap is not measured because no Coverage report was named.

| Source set | Metric | Value | Behind the value | Reference range |
| ---------- | ------ | ----- | ---------------- | --------------- |
| Production | Erosion | 0 | 0 of 5.2915 Function mass in eroded functions | 0.206 to 0.731, source: codeint stratified panel of 50 TypeScript and JavaScript repositories, ADR 0028, codeint 0.6.0 |
| Production | Cognitive complexity | 0 | 0 of 2 cognitive complexity in functions over 15 | no published Reference range for this language |
| Production | Duplication | 0 | 0 clone lines of 7 source lines | 0.006 to 0.194, source: codeint stratified panel of 50 TypeScript and JavaScript repositories, ADR 0028, codeint 0.6.0 |
| Production | Verbosity | 0.1429 | 1 Flagged line of 7 source lines | 0 to 0.001, source: codeint stratified panel of 50 TypeScript and JavaScript repositories, ADR 0028, codeint 0.6.0 |
| Production | Import cycles | 0 | 0 of 1 module in an Import cycle | 0 to 0.184, source: codeint stratified panel of 50 TypeScript and JavaScript repositories, ADR 0028, codeint 0.6.0 |

## Production Findings

### Verbosity

| Flagged line | Rule |
| ------------ | ---- |
| `src/sign.ts:4` | else-after-return |

## Files not measured

- `src/broken.ts`: cannot parse: Unexpected token at line 1, column 14
- `src/types.d.ts`: declaration only
