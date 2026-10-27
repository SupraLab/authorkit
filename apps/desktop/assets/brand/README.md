# AuthorKit brand assets

Source of truth for packaging and the window icon: `icon-512.png`.

| Asset | Platform |
|-------|----------|
| `icon-{16,24,32,48,64,128,256,512}.png` | Window icon, Linux hicolor |
| `AuthorKit.icns` | macOS `.app` (`CFBundleIconFile`) |
| `authorkit.ico` | Windows PE resource + zip side-car |
| `mark-*.png` | In-app quill mark (not the dock icon) |
| `authorkit-banner.png` | Marketing / README |

Regenerate with:

```bash
./scripts/gen-icons.sh
```

Copyright © 2026 Julien Moulin / [SupraLab](https://supralab.fr/en).
