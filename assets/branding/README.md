# codeint branding

This folder holds the codeint mascot, its lockup, the README banner, and the repository social preview.

## Mascot

[mascot.png](./mascot.png) is the codeint mascot: an amber owl that watches Software entropy grow before it compounds.
It is the unchanged 1254 × 1254 output of one draw from the Codex CLI built-in `image_gen` tool, which did not report its model name.
[mascot-prompt.txt](./mascot-prompt.txt) is the exact prompt, written with the IP as Logo skill.
[mascot.webp](./mascot.webp) is a 512 × 512 copy for small displays.

The palette comes from the mascot:

| Role | Color |
| ---- | ----- |
| Ground | `#275e5e` teal |
| Attention | `#f78b27` amber |
| Text | `#fbe9cb` cream |
| Background | `#081112` ink |

## Banner, lockup, and social preview

[source/banner.html](./source/banner.html) and [source/lockup.html](./source/lockup.html) are the editable sources.
The banner panel shows the `codeint check` output of the sample repository in [docs/reference.md](../../docs/reference.md#codeint-check).
Render every image with Google Chrome and `cwebp`:

```sh
assets/branding/source/render.sh
```

The script writes these files into this folder:

- `banner.png` and `banner.webp`: the 1600 × 700 README banner.
- `social-preview.png`: the 1600 × 800 image for the repository social preview.
- `lockup-dark.png` and `lockup-light.png`: the mascot and the wordmark on a transparent background, for dark and light pages.

GitHub sets the social preview only in the repository settings, under Social preview.
