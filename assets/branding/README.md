# codeint branding

This folder holds the codeint mascot, its lockup, the README banner, and the repository social preview.

## Mascot

[mascot.png](./mascot.png) is the codeint mascot: an amber owl that watches Software entropy grow before it compounds.
It is the unchanged 1254 × 1254 output of one draw from the Codex CLI built-in `image_gen` tool.
The tool did not report its model name.
[mascot-prompt.txt](./mascot-prompt.txt) is the exact prompt, written with the IP as Logo skill.

## Banner, lockup, and social preview

[source/banner.html](./source/banner.html) and [source/lockup.html](./source/lockup.html) are the editable sources.
The color palette lives in the `:root` block of `source/banner.html`.
The banner panel is a rendering of the `codeint check` report for the sample repository in [docs/reference.md](../../docs/reference.md#codeint-check).
Each Metric row takes its ratio and Reference range from `data-` attributes, so update those when the reference example changes.

Render the images with [source/render.sh](./source/render.sh).
It needs Google Chrome, `cwebp`, and a network connection for Google Fonts.
Set `CHROME` to the Chrome binary when it is not at the macOS default path:

```sh
assets/branding/source/render.sh
```

The script writes these files into this folder:

- `banner.webp`: the 1600 × 700 README banner.
- `social-preview.png`: the 1600 × 800 image for the repository social preview.
- `lockup-dark.png` and `lockup-light.png`: the mascot and the wordmark on a transparent background, for dark and light pages.

GitHub sets the social preview only in the repository settings, under Social preview.
