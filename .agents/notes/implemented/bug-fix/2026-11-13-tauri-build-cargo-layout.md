# Agent Note: Tauri 构建使用当前 Cargo 目录定位契约

Status: implemented

## Problem

v0.3.22 的 GitHub Windows GNU 门禁在 `tauri_build::try_build` 返回系统错误 3。远程安装的是 Rust 1.100.0，本机通过完整门禁的是 1.98.1。远程 `OUT_DIR` 多出按包名分组的一层，而锁定的 tauri-build 2.6.3 固定向上取三层，将目标目录错认成 `target/debug/build`，随后读取不存在的 `build/build`；这不是应用配置错误。

Tauri 官方 2.7.0 修正目标目录定位，2.7.1 修正新布局中的 Windows GNU WebView2Loader.dll 查找；当前同一主版本的 2.7.4 已包含这些修复。官方来源：[Tauri 构建日志](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-build/CHANGELOG.md)、[Cargo 布局说明](https://blog.rust-lang.org/2026/03/13/call-for-testing-build-dir-layout-v2/)。

## Decision

现有构建依赖最低版本为 tauri-build 2.7.4，锁文件同步其必要依赖。继续由 Tauri 拥有目标目录与 WebView2 资源定位；应用不增加 Cargo 布局推断、目录填补、DLL 复制或安装器补丁，也不通过降级 Rust 避开问题。Tauri 运行时版本与配置格式不变。

保留失败且未发布的 v0.3.22 标签，正式版本使用 v0.3.23 并合并其未发布说明，不重写远程标签。发布仍要求四平台门禁、签名打包和资源检查全部通过。

### Related decisions

- [CI 工具链供给](../../implemented/process/2026-10-14-pinned-ci-toolchain-provisioning.md)：部分重叠，继续拥有 Node 的固定版本和供给；本次不改变 Rust 选择或另建工具链机制。
- [开发缓存边界](../../implemented/process/2026-09-24-local-build-cache-retention.md)：边界相关，缓存清理规则不变，不把目录修复加入清理脚本。
- 活跃笔记中没有同主题提案或被完全吸收的决定。

## Alternatives considered

- 固定旧 Rust 最容易复用本机成功结果，也能提高复现一致性；但当前失败已由 Tauri 上游修复，降级不能修正依赖的目录定位问题，因此采用上游修复。
- 在 build.rs 预建 `build/build` 或自行寻找、复制 DLL，改动看似更小；但会使资源落在错误目录，并复制框架所有权，不能作为完整修复。
- 更新整个 Tauri 运行时可一次获得更多新修复；本问题已有构建依赖的直接修复，无需扩大运行时更新范围。

## Verification

`npm run ci` 覆盖完整工作区测试、前端测试、笔记和生产构建；配置及客户端路径使用隔离目录。GitHub 四个平台在最新 stable 上执行同一门禁、签名打包和资源检查，其中 Rust 1.100 的 Windows GNU 实际构建验证新目录定位，不能由本机 1.98.1 通过代替。发布结果核对标签提交、更新说明、签名资产与更新清单，具体结果保留在忽略目录 `.tmp/release-0.3.23`。

## Consequences

构建依赖携带的代码生成和工具依赖需要重新编译；以完整源码门禁及四平台实际打包验证，不把依赖解析成功当作发布成功。

独立侧栏改动继续保留在工作区，真实安装和配置不参与构建验证。
