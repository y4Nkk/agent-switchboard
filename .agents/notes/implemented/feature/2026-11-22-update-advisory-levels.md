# Agent Note: 应用更新的三档紧迫标签

Status: implemented

## Problem

关于页发现新版本时只有版本号与发布说明，用户无法判断「能等到什么时候再更新」。需要按已确认的三档设计（常规/重要/紧急）给出档位标签，且关键规则是：跳过修复版的用户仍收到提醒，已修复的用户不被重复打扰。

## Decision

档位是发布者明示的事实，不是从更新日志或版本号推导的结论。单一属主是 `release/update-advisories.json`：累计保留的记录数组，每条含 `severity`（`important`/`urgent`）、受影响区间 `[affected_from, fixed_in)`（纯 `X.Y.Z`）、双语原因 `reason_zh`/`reason_en`。`scripts/updater-release.mjs` 的 `readUpdateAdvisories` 在发布时严格校验（键集合、severity 枚举、版本序、非空原因），`generateUpdaterManifest` 把结果写进 `latest.json` 的 `asb_advisories` 字段；字段恒存在，空数组即「发布者声明全部为常规」。

档位判定唯一属主是 `src-tauri/src/app_update/advisory.rs::resolve`：从 Tauri updater 已返回的 `Update.raw_json` 读 `asb_advisories`，以原生 `current_version` 与候选 `version` 匹配。发布脚本与客户端接受同一记录结构。区间端点必须是规范的稳定 SemVer 字符串，每段为无前导零的无符号 64 位整数，按整数精确比较。记录只在「候选已包含修复（`fixed_in <= offered`）且安装版本落在区间内」时匹配，紧急高于重要，同档取最新 `fixed_in`。`UpdateCheck.advisory` 三态穿过既有类型化桥：`None`＝旧清单未表态（不显示标签），`Routine`＝清单无匹配（中性灰「常规更新」），`Important`/`Urgent`＝带双语原因。坏记录阻止发布，客户端逐条跳过坏记录。

关于页在 `UpdateSection` 的版本号旁渲染状态胶囊（复用日志级别胶囊的描边样式：中性灰/警示/错误色），重要与紧急在摘要下一行按当前界面语言显示发布者写的原因；清单未表态时不显示胶囊。侧栏角标、Store 渠道、下载/安装/重启流程与历史日志不变。

## Related decisions

- [GitHub Release 正文的下载表格拼接](../process/2026-11-12-release-download-table.md)：部分重叠，该篇继续拥有 latest.json 的生成与校验流程，本篇只新增 `asb_advisories` 字段事实，已就地同步。
- [关于页可取消的更新下载](2026-11-04-cancellable-update-download.md)：部分重叠，`UpdateCheck` 契约由该篇拥有，本篇为其增加 `advisory` 字段，已就地同步。
- [更新完成后确认重启](2026-11-16-update-restart-confirmation.md)：相邻，档位不改变安装后的确认重启流程。

## Alternatives considered

- 从 CHANGELOG 的「重要变更」标题或版本号跳跃推导档位：零维护成本，但 CHANGELOG 描述的是「这版改了什么」，说不出「哪个安装版本受影响」，推导会猜错；改为发布者明示区间。
- 按发布版本独立标注（每版一条、不累计）：记录更少，但客户端只读最新清单，跳过修复版的用户在新候选上会丢失提醒；改为累计区间加 `fixed_in <= offered` 判定。
- 侧栏角标同步分色、紧急弹窗提醒：可见性更高，但档位是建议不是阻断，不引入未请求的强制打断；关于页详情 + 现有绿色角标已回答「有没有新版」。
- 客户端拉取第二份目录端点判定档位：可绕开 latest.json 体积，但产生第二个发布事实来源与一次额外网络请求；`Update.raw_json` 已保留原始响应，直接内嵌即可。
- 把档位编码进版本号（如 `-urgent` 后缀）：对用户可见且破坏纯稳定版本序；档位走清单字段。

## Consequences

- 收益：跳过修复版的用户在任意后续候选上仍看到正确档位与原因；已修复安装回落常规；发布者只需维护一份累计清单，坏记录在发布时阻断而不是在线上静默。
- 代价：每次发布都要带 `--advisories-file`（缺失即失败，属有意的显式契约）；区间写错（如 `fixed_in` 晚于实际修复版）会让提醒推迟一个版本才出现；并行发布线（当前不存在）会使 `fixed_in <= offered` 不再充分，届时需按发布线拆分记录。
- 兼容：无 advisory 读取能力的已装客户端忽略新字段；新客户端对旧清单显示无标签，不误报常规。
