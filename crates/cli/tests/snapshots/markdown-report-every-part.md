# codeint report

codeint measured 26 files with 498 source lines: Production has 25 files with 496 source lines, Test has 1 file with 2 source lines.

| Source set | Metric | Value | Behind the value | Reference range |
| ---------- | ------ | ----- | ---------------- | --------------- |
| Production | Erosion | 0.9976 | 2177.4789 of 2182.7704 Function mass in eroded functions | no published Reference range for this language |
| Production | Duplication | 0.9738 | 483 clone lines of 496 source lines | no published Reference range for this language |
| Production | Verbosity | 0.9758 | 484 verbose lines of 496 source lines: 1 Flagged line, 483 clone lines | no published Reference range for this language |
| Production | Import cycles | 0.08 | 2 of 25 modules in an Import cycle, 1 unresolved import | no published Reference range for this language |
| Test | Erosion | 0 | 0 of 0 Function mass in eroded functions | no published Reference range for this language |
| Test | Duplication | 0 | 0 clone lines of 2 source lines | no published Reference range for this language |
| Test | Verbosity | 0 | 0 verbose lines of 2 source lines: 0 Flagged lines, 0 clone lines | no published Reference range for this language |
| Test | Import cycles | 0 | 0 of 1 module in an Import cycle | no published Reference range for this language |

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

- 60 tokens: `src/f01.ts:1-7`, `src/f02.ts:1-7`, `src/f03.ts:1-7`, `src/f04.ts:1-7`, `src/f05.ts:1-7`, `src/f06.ts:1-7`, `src/f07.ts:1-7`, `src/f08.ts:1-7`, `src/f09.ts:1-7`, `src/f10.ts:1-7`, `src/f11.ts:1-7`, `src/f12.ts:1-7`, `src/f13.ts:1-7`, `src/f14.ts:1-7`, `src/f15.ts:1-7`, `src/f16.ts:1-7`, `src/f17.ts:1-7`, `src/f18.ts:1-7`, `src/f19.ts:1-7`, `src/f20.ts:1-7`, `src/f21.ts:1-7`
- 54 tokens: `src/f01.ts:2-7`, `src/f02.ts:2-7`, `src/f03.ts:2-7`, `src/f03.ts:8-13`, `src/f04.ts:2-7`, `src/f04.ts:8-13`, `src/f05.ts:2-7`, `src/f05.ts:8-13`, `src/f06.ts:2-7`, `src/f06.ts:8-13`, `src/f07.ts:2-7`, `src/f07.ts:8-13`, `src/f08.ts:2-7`, `src/f08.ts:8-13`, `src/f09.ts:2-7`, `src/f09.ts:8-13`, `src/f09.ts:14-19`, `src/f10.ts:2-7`, `src/f10.ts:8-13`, `src/f10.ts:14-19`, `src/f11.ts:2-7`, `src/f11.ts:8-13`, `src/f11.ts:14-19`, `src/f12.ts:2-7`, `src/f12.ts:8-13`, `src/f12.ts:14-19`, `src/f13.ts:2-7`, `src/f13.ts:8-13`, `src/f13.ts:14-19`, `src/f14.ts:2-7`, `src/f14.ts:8-13`, `src/f14.ts:14-19`, `src/f15.ts:2-7`, `src/f15.ts:8-13`, `src/f15.ts:14-19`, `src/f15.ts:20-25`, `src/f16.ts:2-7`, `src/f16.ts:8-13`, `src/f16.ts:14-19`, `src/f16.ts:20-25`, `src/f17.ts:2-7`, `src/f17.ts:8-13`, `src/f17.ts:14-19`, `src/f17.ts:20-25`, `src/f18.ts:2-7`, `src/f18.ts:8-13`, `src/f18.ts:14-19`, `src/f18.ts:20-25`, `src/f19.ts:2-7`, `src/f19.ts:8-13`, `src/f19.ts:14-19`, `src/f19.ts:20-25`, `src/f20.ts:2-7`, `src/f20.ts:8-13`, `src/f20.ts:14-19`, `src/f20.ts:20-25`, `src/f21.ts:2-7`, `src/f21.ts:8-13`, `src/f21.ts:14-19`, `src/f21.ts:20-25`, `src/f21.ts:26-31`
- 53 tokens: `src/f01.ts:2-8`, `src/f02.ts:2-8`, `src/f03.ts:2-8`, `src/f04.ts:2-8`, `src/f04.ts:8-14`, `src/f05.ts:2-8`, `src/f05.ts:8-14`, `src/f06.ts:2-8`, `src/f06.ts:8-14`, `src/f07.ts:2-8`, `src/f07.ts:8-14`, `src/f08.ts:2-8`, `src/f08.ts:8-14`, `src/f09.ts:2-8`, `src/f09.ts:8-14`, `src/f10.ts:2-8`, `src/f10.ts:8-14`, `src/f10.ts:14-20`, `src/f11.ts:2-8`, `src/f11.ts:8-14`, `src/f11.ts:14-20`, `src/f12.ts:2-8`, `src/f12.ts:8-14`, `src/f12.ts:14-20`, `src/f13.ts:2-8`, `src/f13.ts:8-14`, `src/f13.ts:14-20`, `src/f14.ts:2-8`, `src/f14.ts:8-14`, `src/f14.ts:14-20`, `src/f15.ts:2-8`, `src/f15.ts:8-14`, `src/f15.ts:14-20`, `src/f16.ts:2-8`, `src/f16.ts:8-14`, `src/f16.ts:14-20`, `src/f16.ts:20-26`, `src/f17.ts:2-8`, `src/f17.ts:8-14`, `src/f17.ts:14-20`, `src/f17.ts:20-26`, `src/f18.ts:2-8`, `src/f18.ts:8-14`, `src/f18.ts:14-20`, `src/f18.ts:20-26`, `src/f19.ts:2-8`, `src/f19.ts:8-14`, `src/f19.ts:14-20`, `src/f19.ts:20-26`, `src/f20.ts:2-8`, `src/f20.ts:8-14`, `src/f20.ts:14-20`, `src/f20.ts:20-26`, `src/f21.ts:2-8`, `src/f21.ts:8-14`, `src/f21.ts:14-20`, `src/f21.ts:20-26`
- 53 tokens: `src/f01.ts:6-13`, `src/f02.ts:7-14`, `src/f03.ts:8-15`, `src/f04.ts:9-16`, `src/f05.ts:10-17`, `src/f06.ts:11-18`, `src/f07.ts:12-19`, `src/f08.ts:13-20`, `src/f09.ts:14-21`, `src/f10.ts:15-22`, `src/f11.ts:16-23`, `src/f12.ts:17-24`, `src/f13.ts:18-25`, `src/f14.ts:19-26`, `src/f15.ts:20-27`, `src/f16.ts:21-28`, `src/f17.ts:22-29`, `src/f18.ts:23-30`, `src/f19.ts:24-31`, `src/f20.ts:25-32`, `src/f21.ts:26-33`

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
