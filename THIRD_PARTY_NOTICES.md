# Third-party notices

## zapret-discord-youtube

- Repository: https://github.com/Flowseal/zapret-discord-youtube
- Author/maintainer: **Flowseal**
- License: MIT
- Copyright: © 2016–2026 bol-van; © 2024–2026 Flowseal

`zapret-ui` is an independent GUI manager. It is **not** an official client of
Flowseal or bol-van and does not include the zapret runtime in its own release
archives; the runtime is downloaded separately from the upstream GitHub releases.

The upstream MIT license text:

```text
MIT License

Copyright (c) 2016-2026 bol-van
Copyright (c) 2024-2026 Flowseal

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## WinDivert

- Repository: https://github.com/basil00/WinDivert
- Author: **basil00**
- License: LGPL-3.0 or GPL-2.0 (at your choice)

WinDivert is distributed as part of the upstream zapret-discord-youtube runtime,
not as part of `zapret-ui`. The upstream runtime and WinDivert are downloaded
unmodified from their respective releases. Source code and license terms:
https://github.com/basil00/WinDivert

## zapret (bol-van)

- Repository: https://github.com/bol-van/zapret
- License: MIT

The upstream Flowseal runtime contains binaries originating from bol-van/zapret.
Their copyright notice is retained above.

## Flowseal/tg-ws-proxy (Telegram WS proxy)

- Repository: https://github.com/Flowseal/tg-ws-proxy
- Author/maintainer: **Flowseal** and contributors
- License: MIT

The Telegram proxy binary (`tgws-cli-windows-x64.exe`) is **not** bundled in the
`zapret-ui` release archives: the app downloads it on demand from this
repository's `tgws-v*` releases into `%LOCALAPPDATA%\zapret-ui\tgws\`.

The binary is built in GitHub Actions from the **unmodified** upstream sources at
the tag named in the release (`tgws-v1.11.2-1` → upstream `v1.11.2`). The only
addition is a four-line launcher (`scripts/tgws/entry.py`) that calls the
upstream console entry point `proxy.tg_ws_proxy:main`, so the frozen executable
starts headless instead of creating a tray icon and GUI dialogs
(`scripts/tgws/tgws-cli.spec`). Upstream's own Windows release remains the
tray application and is not redistributed here.

The upstream MIT license text:

```text
MIT License

Copyright (c) Flowseal and tg-ws-proxy contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
