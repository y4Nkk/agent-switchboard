# Agent Note: GitHub Release 正文的下载表格拼接

Status: implemented

## Problem

GitHub Release 正文此前只有 CHANGELOG.md 提取的版本段落，用户要在资产列表里从 `*.sig`、`*.app.tar.gz`、`latest.json` 等更新器载荷中挑出真正该下载的安装包。需要一个按操作系统分行、徽章按钮只指向用户可见安装包的图形化下载表格，并随发布自动生成（参照 FlClash 的发布页做法：shields.io 徽章 + Markdown 表格由 CI 拼进正文）。

## Decision

下载表唯一属主是 `.github/release_template.md`：动态下载量徽章（`img.shields.io/github/downloads/REPOSITORY/vVERSION/total`）、五枚静态徽章套资产链接的表格、CHANGELOG 链接；模板不含任何仓库与版本字面量，`REPOSITORY` 与 `VERSION` 两个占位符由 `package.yml` 的 release job 注入（`GITHUB_REPOSITORY` 与 tag 派生版本），仓库身份单一来源是 GitHub 上下文。资产文件名与 `scripts/updater-release.mjs` 的 `BUILD_ARTIFACTS` 命名逐字一致（nsis.exe、darwin-aarch64.dmg、darwin-x86_64.dmg、linux-x86_64.AppImage、linux-x86_64.deb）。拼接发生在「生成签名更新清单」之后、「创建或复用 Release」之前——顺序承重：manifest 的 `--notes-file` 必须吃到纯 changelog（应用内更新说明不含徽章表格），create/edit 两条发布路径读的都是拼接后的正文。表格只列用户直接下载的资产；`*.app.tar.gz`、`*.sig`、`latest.json` 是更新器载荷，永不进表。

## Release metadata

发布说明中的发布日期唯一来自 `CHANGELOG.md` 的版本标题（`## [x.y.z] - YYYY-MM-DD`）。`readReleaseNotes` 校验实际日历日期并在正文前展示，不以构建时间补值；GitHub 正文与应用更新说明消费同一输出。日期缺失、格式错误或空正文阻止生成发布说明。更新器 `pub_date` 仍表示清单的生成时间，不冒充版本日历日期。

发布版本校验同时检查 `Cargo.toml`、标签与三个 Rust 工作区包的锁文件版本。只替换 `Cargo.lock` 中第一个版本会留下其余 `version.workspace` 包的旧值，使四平台 `--locked` 门禁失败；发布时使用 Cargo 同步工作区版本，保持第三方依赖版本与校验和不变。失败的 `v0.3.19` 标签不改写，其未完成发布的内容合并入 `v0.3.20`。

## Related decisions

- [应用内更新日志渲染](../feature/2026-10-14-changelog-section-display.md)确立 CHANGELOG.md 是更新说明唯一来源——本篇不改变它：release 正文说明部分仍由 `updater-release.mjs release-notes` 从 CHANGELOG.md 提取，表格只是追加段，latest.json 保持纯 changelog。

## Alternatives considered

- 手动在 Release 编辑框粘贴表格：零 CI 成本，但每次发布都要手工替换版本号并核对文件名，漏改即死链；模板 + sed 让文件名契约只写一次。
- 徽章直链 `releases/latest/download/…` 而非固定 tag：URL 永不变，但历史版本页面的徽章会指向最新版文件，版本错位；固定 tag 链接与页面自洽。
- 把下载表也塞进 latest.json 的更新说明：应用内查看器禁原生 HTML/图片，徽章渲染不出且噪音大。
- 用发布时的本机日期填正文最省步骤，但会因重跑流水线改变历史版本日期；从版本标题提取使日期有唯一所有者，并使缺失日期显式失败。

## Consequences

- 收益：发布页按操作系统图形化选择下载；下载量徽章由 shields.io 动态统计。
- 代价：新增资产（如 ARM Windows 包）必须同步改 `release_template.md`（文件名与徽章两处）；下载量统计含 `.sig` 等更新器载荷，数字偏大是已知接受项；sed 替换依赖正文中不出现其他 `REPOSITORY`/`VERSION` 字面量。

## Verification

- 五个占位文件名经 sed 干跑（v0.3.18）后与线上真实资产 `gh api` 逐字核对，全部命中。
- 现发布 body 与「tag 本体 CHANGELOG 经 `release-notes` 提取」逐字节一致（无手工编辑），组合正文仅追加表格段后经 `gh release edit` 回填 v0.3.18。
- 浏览器实测：中文徽章 SVG 直开渲染正常（安装版 x64）；线上 Release 页实页截图确认下载量徽章、三行六枚徽章与 CHANGELOG 链接全部渲染。shields.io 经 GitHub camo 代理，终端 curl 不通不影响页面。
