# Agent Note: 双客户端安装识别、版本对比与显式更新

Status: implemented

## Problem

关于页拥有 Agent Switchboard 自身更新。Codex 雷达的 cliVersion 是一次检测的历史快照，不能代替当前安装状态；Claude Code 也需要独立的版本管理入口。仅比较 PATH 中的版本与一个标签，会混淆多份安装、桌面内置引擎、编辑器扩展和独立 CLI；统一执行 npm 更新还会改变安装归属。

## Decision

「设置 → 客户端管理 → 软件版本」管理当前原生系统中的 Codex CLI 和 Claude Code CLI，提供发现、版本对比、更新预览、显式执行与结果复验。桌面应用、编辑器扩展、受保护或未知归属的安装提供引导。WSL、容器和 SSH 目标不由宿主系统更新器接管。

src-tauri/src/client_software 是安装实例、发布快照、可更新性、计划与任务的唯一所有者。真实安装由已识别实例的原安装器或原包管理器负责；关于页 app_update 保留 ASB 自身更新的独立契约。两者共享安装互斥门，不扩展为通用更新框架。

实现不安装新的客户端、不迁移旧未知布局、不修改上游安装器、不引入版本事实的永久缓存或兼容读取。产品事实在 README.md / README.zh-CN.md，界面事实在 DESIGN.md，本篇只记录边界与取舍。

## Installation ownership

- discovery 从绝对 PATH、已知入口与用户通过原生文件选择器明确选择的位置识别安装。选择的额外位置仅在本次应用运行内保存，不改变默认启动入口。符号链接与 junction 解析后按客户端、形态和稳定安装根去重，保留多个实例及「ASB 当前调用」。这个标记不代表外部终端的 alias 或项目环境。
- identity/packages 与 identity/installations 拥有共同的纯安装证据；resource_monitor/executables 只将这些证据投影到资源监控分组。原私有安装/包判断文件移至共同所有者，不保留两套匹配表。资源监控继续拥有进程用户、出生标识、拓扑、CPU 与内存。
- Codex 原生安装核实当前 codex-package.json、layoutVersion 1、codex variant、bin 入口与 packages/standalone 根。自定义 CODEX_HOME 可通过实际布局识别。直接选择版本目录中的载荷只展示引导，更新必须使用稳定的外部入口。
- Claude 原生安装区分用户版本目录、标准原生入口、npm 平台包、Homebrew 包与桌面下载缓存。`identity/installations::claude_version_entry` 唯一拥有版本目录直接入口的数字版本文件名规则；更新归属额外要求原生文件证据，目录下的任意文件或嵌套路径不能被认领。稳定入口与版本载荷按启动入口的规范父目录区分，带 `..` 的路径也不能绕过直接选择载荷的限制。桌面、扩展或专用内置引擎不会成为独立 CLI 的更新目标。
- npm 解析主包的真实 bin、声明的平台包、global prefix 与 Node/npm 配对。CLI 启动只使用已解析入口；查询版本仓库才通过同一 npm 读取 scoped/default registry。原有 registry 和认证处理留给该 npm，不改 .npmrc，不强制切公共源。已识别的 pnpm、Bun、Yarn 等其他管理器只引导，不用 npm 接管。
- Homebrew 用实际 Caskroom、原 brew、精确 cask 和 .metadata/INSTALL_RECEIPT.json 的 tap/version 核实归属；只自动执行确认来自 homebrew/cask 的当前安装，不把未知 tap 当成官方渠道。
- WinGet 使用官方部署 COM/WinRT API，不解析本地化表格。精确关联 OpenAI.Codex / Anthropic.ClaudeCode 的目录、当前用户范围和官方源的 portable 目录证据；仅有系统注册关联不构成接管资格。执行时再次核对目录与范围。
- 系统目录、共享存储、权限不足与未确认管理器展示当前能读取的版本及官方更新说明，交由原渠道或管理员处理，不推测系统包名或执行提权命令。
- Windows 桌面读取当前用户应用包与卸载记录，macOS 读取已知 Applications 目录；编辑器读取已知扩展根的准确 publisher/name。它们展示宿主/扩展版本。任意便携、不可访问、未挂载位置不宣称完全发现。

Windows 安装身份继续使用规范路径；Node 的客户端、npm JavaScript 入口和 npm `--prefix` 参数在共同 `process::command_path` 边界转换为普通驱动器或 UNC 路径，避免模块加载器误解析扩展长度前缀，以及 npm Arborist 在安装目录祖先解析中因该前缀无限递归。CLI 启动、npm 仓库读取、版本查询与真实安装使用同一投影；删除启动模块私有转换，原 Node/npm 与安装目录的身份仍保持规范路径，不改变实际安装目标、归属比对或其他平台的原始路径。仅让夹具避开规范路径虽然改动更少，但会隐藏真实 npm 客户端同样存在的启动失败，因此修复实际启动边界。

## Version facts and policy

CLI 界面分别保存本机版本、官方最新与当前渠道可用版本。版本原文保留在详情，比较由 Rust semver 唯一完成。未知或过期不等于已最新；本机领先不执行降级；当前渠道已最新但官方更高会显示等待渠道发布。

Codex 官方源为 https://releases.openai.com/codex/channels/latest；Claude 为 https://downloads.claude.ai/claude-code-releases/latest，原生 stable 使用对应 /stable。npm 候选由原工具链的原 registry 查询指定标签；Homebrew 读取精确 cask 的官方 JSON；WinGet 查询官方包目录的当前候选。来源失败保留原因与过期标记，不静默换渠道。

发布读取使用进程内一小时缓存，手动检查强制刷新；来源最多两个并发。代理修订变化使缓存失效。隐藏页面停止轮询；重新可见时读本地及后端任务，再按缓存策略查询发布源。没有启动应用即全局轮询的第二条路径。

Claude 策略由现有 client_settings 下的 software_policy 只读投影拥有：autoUpdatesChannel、minimumVersion、requiredMinimumVersion / requiredMaximumVersion、DISABLE_UPDATES 和 DISABLE_AUTOUPDATER。用户文件、管理文件及片段、Windows 管理注册表按来源规则处理，env 按键合并；坏文档和未知值明确阻止自动更新。存在 MDM、policyHelper 或强制远端策略刷新线索时引导核实，不把无法解析当成允许。

本地投影不宣称复刻远端组织策略或 Claude 全部设置合并器；Claude 原生更新器仍拥有最终执行规则。仅关闭后台自动更新不禁止手动更新。版本页不新增这些配置的写入入口。

## Execution and recovery

| 安装方式 | 执行与目标 |
| --- | --- |
| npm | 同一 Node/npm、prefix 和 registry 安装明确版本；预先核对目标包身份和 Node 要求 |
| Codex 原生 | 下载官方原始脚本，绑定现有 CODEX_HOME、稳定入口与明确版本，非交互执行；临时 PATH 优先选中实例 |
| Claude 原生 | 通过选中入口执行 update；跟随原渠道，完成后核实实际版本和当前政策 |
| Homebrew | 原 brew upgrade --cask --greedy 和原 cask；不换 stable/latest，不加忽略固定版本选项 |
| WinGet | 官方 UpgradePackageAsync 绑定精确 PackageVersionId、原目录与用户范围；不绕过哈希、源或权限规则 |

npm、Codex 原生与 WinGet 采用精确版本；Claude 原生和 Homebrew 跟随执行时渠道，预览明确候选可能变化。平台选择、包完整性、签名与依赖维护交给原安装器，失败不能被包装成更新完成。预览说明安装器可能维护 PATH、入口与依赖；ASB 不把供应商、认证或会话配置作为本次更新目标。

计划只由后端生成，五分钟失效，绑定实例、原入口、实际旧版本、执行归属、文件观测与清单摘要、渠道、策略和代理修订。开始任务及提交安装前都复核；变更则要求重新准备，不悄悄重定向到其他实例。

子进程代理使用仅本次进程的环境；Windows Codex 通过独立调用脚本设置 .NET 代理，下载的官方脚本保持原样。其手动 SOCKS 场景明确拒绝并给出 HTTP/官方指引。WinGet 的系统服务网络独立显示，不宣称受 ASB reqwest 代理控制。

全应用同一时刻只有一个软件安装任务。更新相关客户端时阻止新的 ASB 雷达和会话启动；原生进程检查也覆盖 npm 的关联平台包。已有进程提示用户结束，不强杀用户会话；占用只由候选进程可证实的运行映像裁决，读不到映像路径的进程不阻断，其漏拦风险不等于安装器必然报文件锁错误（见[更新占用守卫](../bug-fix/2026-11-09-update-occupancy-guard.md)）。ASB 发起的会话和雷达使用共同启动解析器，历史 cliVersion 仍是历史快照。

状态为 preparing → runningUpdater → verifying → completed / failed / needsAttention，准备取消进入 cancelled。进入安装阶段前取消请求与提交通过同一锁协调；提交后不提供强杀取消，不伪造下载百分比或通用回滚。

更新进程使用启动握手：包装器先等启动信号，父进程持久化 PID 与出生标识后才放行；父进程在放行前退出，输入 EOF 让包装器自行结束。更新不复用雷达的 kill-on-parent-exit JobObject，也不设安装强杀超时。正常退出被阻止；意外退出仍可能导致安装部分完成，必须实测恢复。

Windows PowerShell 5 包装器直接接收 `ConvertFrom-Json` 返回的参数数组，不再用 `@(...)` 创建嵌套数组；每个参数单独按 Windows 原生命令规则转义后交给 `ProcessStartInfo`。子进程输入关闭为 EOF，标准输出和错误各自异步读取并按原字节转发，读完后返回原退出码；ASB 意外退出导致某条输出管道断开时，该流继续排空，避免安装器被写满的管道卡住。否则 Node 会把 npm 入口与所有选项当成一个模块路径，且未显式重定向的孙进程输出会丢失；这两个问题共同造成安装根本没开始、日志为空而版本复验仍为旧版。该边界由所有 Windows 客户端更新渠道共用，不增加安装强杀、自动重放或第二套更新器。

唯一持久记录为 state/client-software/operation.json，采用 version 2 schema、原子替换与同步落盘，保留最后一个任务及有界、脱敏输出。error 为共享 LocalizedMessage 或 null，占用错误在预览、启动提交与安装前复验中保留同一客户端/PID 参数，随界面语言渲染。[操作日志当前契约](../architecture/2026-11-09-current-operation-journals.md)接管严格读取边界：旧版本、字符串错误及非法结构明确拒绝并保留原件，不在读取时转换或重写；读写共用 256 KiB 大小上限。任务 revision 跨操作递增，前端拒绝旧响应覆盖新结果。重启不重放命令，先等待原 PID/出生身份或 WinGet 安装任务结束，再检查同一入口、归属与真实版本；PID 复用不等于原任务仍在运行。进程退出码为零不能替代复验。

## Boundaries and interface

Tauri 与开发桥共同注册 read_client_software、select_client_software、refresh_client_software、prepare_client_update、start_client_update、read_client_update、cancel_client_update。选择位置由后端系统对话框返回，前端不能提交任意路径、Shell、包名、下载地址或提权参数。

React 版本区使用既有 Table / Button / AppDialog / FactList / Time / CollapsibleSection / CodePreview 和双语词典。Codex 与 Claude 的安装列表共用横向 Table，每行一份安装，对齐本机、官网、当前渠道版本、渠道、状态和动作；完整路径与来源使用共享行内展开，详情、更新计划和单次任务的键值事实仍由 FactList 拥有。表格仅拥有横向溢出，纵向随设置页展开，不保留安装卡片的平行渲染路径。更新计划与任务独立于所选客户端页签，切页继续执行；失败任务默认展开已有安装器输出，用户仍可收起，新任务重置展开选择。失败保留旧读数、来源时间与诊断，不把探测失败显示为未安装。

雷达启动回归的测试路径覆盖只选择隔离夹具入口，仍复用真实安装身份解析，不直接伪造手动安装。Windows 夹具声明 npm 包名与 JavaScript 入口，经已确认的 Node/npm 运行环境启动；未知 .cmd/.bat 入口继续拒绝执行。这样回归覆盖实际归属与启动链，避免测试与生产采用不同命令规则。

## Upstream evidence

研究与实现对照日期为 2026-11-04；版本样本不是产品常量，也不是本机安装成功证据。

- [Codex 安装与更新](https://developers.openai.com/codex/cli)、[Windows 官方安装脚本](https://github.com/openai/codex/blob/main/scripts/install/install.ps1)、[Unix 官方安装脚本](https://github.com/openai/codex/blob/main/scripts/install/install.sh)：当前独立包、稳定入口、非交互参数、版本/平台校验及安装器影响。
- [Codex npm 入口](https://github.com/openai/codex/blob/main/codex-cli/bin/codex.js)：主包、平台包和管理器归属不同；不能按 Node 进程存在与否认定安装方式。
- [Claude 安装](https://code.claude.com/docs/en/setup)、[环境变量](https://code.claude.com/docs/en/env-vars)、[设置](https://code.claude.com/docs/en/settings-reference)：渠道、手动更新和版本限制。
- [Homebrew Cask 元数据](https://github.com/Homebrew/brew/blob/master/Library/Homebrew/cask/metadata.rb)、[安装收据](https://github.com/Homebrew/brew/blob/master/Library/Homebrew/cask/tab.rb)、[公开 API](https://formulae.brew.sh/docs/api/)：当前 receipt 的来源/版本及查询渠道。
- [WinGet 官方接口](https://github.com/microsoft/winget-cli/blob/master/src/Microsoft.Management.Deployment/PackageManager.idl)：InstalledVersion、AvailableVersions、UpgradePackageAsync 及安装范围。接口 winmd 的来源、SHA-256 和许可证在 src-tauri/vendor/winget/README.md；build.rs 用固定 windows-bindgen 生成私有绑定，不依赖用户安装 SDK，不附带安装器。
- 研究中公开元数据样本为 Codex 0.166.0、alpha 0.168.0-alpha.1，Claude latest 2.1.318、stable 2.1.311；Claude 2.1.318 平台 manifest 也已只读取得。部分随后重查的 Codex URL 出现传输 EOF，这正是产品必须保留失败状态的情况，不能据此推断安装结果。

## Related decisions

本主题检索的相邻事实继续保留，没有完全取代的旧笔记：

- [关于页可取消更新](2026-11-04-cancellable-update-download.md)：只共享安装互斥与事实展示原则，不合并更新器。
- [原生进程资源监控](../architecture/2026-10-23-native-process-resource-monitor.md)：纯身份读取移至共同所有者，进程采样责任不变；路径事实同步更新。
- [出站代理](../architecture/2026-10-15-egress-policy-and-quota-failure-contract.md)：复用 HTTP 政策，单独明确外部进程与 WinGet 服务的边界。
- [客户端配置扩展](2026-11-04-current-client-settings-expansion.md)：版本页消费配置投影，不建立第二个配置写入器。

## Alternatives considered

- 两个客户端统一执行 npm install -g：实现简单且 npm 是官方渠道；但会接管原生/Homebrew/WinGet 安装、创建第二份或改变 PATH，所以只按已确认原归属执行。
- 直接下载二进制覆盖：容易绑定版本并显示下载进度；但会破坏包资源、签名和管理器记录，也接管恢复责任，所以复用原安装器。
- 复用 ASB Tauri updater：已有下载、签名与取消；但其信任及重启语义属于 ASB 自身，不能处理外部 CLI 的安装生命周期。
- 只显示本地与官方两个数字：界面最简；但会把稳定渠道、仓库延迟和较新预发布误报为必须更新，所以保留三种事实。
- WinGet 文本或额外 PowerShell 模块：接入方便；前者受本地化影响，后者修改用户环境，所以采用官方部署 API 和随源码固定的接口元数据。
- 常驻更新服务、自动队列及通用回滚：能跨应用退出编排；当前显式更新不需要额外服务，且第三方安装器没有统一事务回滚协议，因此保留单任务、退出保护和最小操作记录。
- Windows 挂起进程后记录身份：能把执行推迟到落盘后；但父进程在放行前崩溃会遗留永久挂起进程，所以改为输入握手，放行前 EOF 自动结束。
- Windows 包装器改用 PowerShell 调用运算符：参数转交和输出转发代码更少；但 Windows PowerShell 5 会再次处理原生参数中的引号与空值，因此保留既有逐参数转义，修正 JSON 数组归属，并显式连接标准流。
- 对两条标准流直接 `CopyToAsync` 后等待安装退出：代码最少且可以并行转发；但父进程退出后复制任务会在写入断管道时失败，剩余输出无人读取会阻塞仍在运行的安装器。使用两个有界缓冲的异步读取，输出断开仅停止该流转发，仍持续排空，不改变安装器生命周期。
- 每份安装使用纵向字段表：长路径与窄窗更易阅读；但多份安装重复表头、版本不能跨行对齐，故主列表采用横向表格，长字段留在行内详情，不为两种布局维护两条路径。

- 测试直接伪造安装类型或放行测试脚本：夹具简单，且不依赖完整包布局；但会绕过真实归属和启动规则，使 Windows 回归失去这条边界的验证，因此夹具声明合法 npm 安装并复用真实解析。

## Consequences

- 收益是安装身份、实时版本和更新执行只有一个事实所有者；多安装、渠道、政策、占用与中断都能显示可操作结果。删除的旧路径仅是重复的安装身份判断与实时 CLI 查找，历史快照和既有配置执行器保持原有职责。
- 代价是需要跟踪官方包布局、发布接口与 WinGet ABI；未知版本或当前契约变化显式失败，不增加宽松解析、旧布局兼容或静默换源。原安装器仍可能影响自身文件、PATH 与依赖，不承诺跨进程崩溃后的原子回滚。
- 新增回归用例覆盖语义版本、渠道解析、版本界限、npm 原目录/运行时、原生稳定入口、任务记录、取消提交边界、启动握手、跨流日志脱敏及迟到响应。用例仅使用隔离临时目录，不更新开发者真实安装。
- Windows npm 的构建与真实升级验收见下节。其他平台及原生、Homebrew、WinGet 渠道的真实升级不在本次验收范围内；不能将 npm 结果外推到这些渠道。
- 2026-11-10 的只读排查确认更新元数据查询成功，但安装命令没有启动且任务输出为空。相同 Windows PowerShell 5 包装器的 `npm --version` 从退出 1 修正为退出 0；后续真实安装进一步暴露规范 npm prefix 触发 Arborist 无限递归。入口与 prefix 现在共同投影，不能仅凭版本查询命令成功判断真实安装可用。
- 手动真实更新验收由 `task/live_tests.rs` 的 ignored 用例拥有，常规测试不下载或安装客户端。用例先确认 USERPROFILE、HOME、APPDATA、LOCALAPPDATA、客户端目录与 npm 配置/缓存均在指定隔离根内，再在带空格的独立 npm prefix 安装旧官方包；版本候选从该 npm 的原 registry 读取，实例经生产识别器确认，更新走真实 prepare/start、启动握手、日志、结果复验和操作记录。用例只向隔离根保存证据，不伪造安装归属或 updater 成功，也不调用用户原安装的更新命令。测试进程保留只读工具链路径和既有构建缓存，用户配置与凭据不复制进隔离目录。

## Verification

2026-11-10 在 Windows 隔离用户目录与 npm prefix 中执行真实官方包升级：Codex `0.162.1 → 0.167.0`、Claude Code `2.1.275 → 2.1.321` 均完成。目标 CLI 的实际返回版本、npm 包清单、原安装根及安装身份、任务 `Completed` 状态与持久化 journal 一致；两款更新均捕获到真实 npm 操作输出。Node 使用仓库固定的 `22.23.2`，npm 为同一工具链的 `10.9.8`。

包含两个手动真实升级用例的 `client_software::` 聚焦检查共 25 项通过、0 失败、0 忽略；其中包含参数转义、双流输出、原退出码、断开输出接收方后仍能完成、安装身份与严格 journal 读取。前端状态与双语词典的 9 项检查以及含类型检查的前端生产构建通过。

Rust 验收采用 HEAD 基线叠加本次更新相关代码的独立源码快照，避开工作树中并行会话改动的编译错误；结果不覆盖这些并行改动。验收后的逐文件核对确认当前更新实现与测试快照一致；并行工作仅在 `mod.rs` 增加 `command_path/output` 重导出，此非行为差异单独记录。隔离目录为 `.tmp/client-software-update-20261110-772d`，内含源码清单、完整测试日志、两端更新结果和汇总。测试未更新用户真实客户端安装或配置，桌面界面交互仍未进行浏览器验收。
