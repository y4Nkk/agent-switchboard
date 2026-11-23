# WenQuanYi Zen Hei 0.9.45

Copyright © 2007–2010 Qianqian Fang and The WenQuanYi Project Board of Trustees.

The font is distributed under the [GNU General Public License version 2](COPYING) with the font embedding exception described in the [upstream README](UPSTREAM-README.txt). The exception also applies to the WOFF2 conversion. See the upstream README for contributor and component credits.

The [corresponding source archive](fonts-wqy-zenhei_0.9.45.orig.tar.gz) is the unmodified upstream release archived by [Debian](https://packages.debian.org/source/bookworm/fonts-wqy-zenhei). Its SHA-256 is `e4b7e306475bf9427d1757578f0e4528930c84c44eaa3f167d4c42f110ee75d6`.

On 2026-11-23, the Agent Switchboard contributors extracted the proportional Zen Hei face (index 0) and encoded it as a standalone WOFF2 for the application. The conversion keeps the face’s character coverage. From the repository root, regenerate it with Python, FontTools 4.61.1, and Brotli 1.2.0.

```sh
python src-tauri/vendor/wqy-zenhei/convert.py src/assets/fonts/wqy-zenhei.woff2
```
