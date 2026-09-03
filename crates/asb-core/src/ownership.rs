//! Ownership directory: the single authority for which configuration keys
//! Agent Switchboard is allowed to read, patch, or write.
//!
//! Every key outside these sets is host-owned and must be preserved
//! byte-for-byte. Patches referencing a host-owned key are rejected at
//! validation time, never silently dropped.

use crate::contracts::{AppKind, CommonSettingValue, CommonSettings, ConfigValue};
use std::collections::BTreeMap;

/// The one ownership decision for a client configuration key. Unknown keys
/// are host-owned by default and therefore never enter a provider file or a
/// common-settings projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingOwner {
    Provider,
    Common,
    Host,
}

/// How one official configuration family is exposed by Agent Switchboard.
///
/// This is deliberately broader than [`SettingOwner`]. `SettingOwner` answers
/// whether a concrete key participates in a provider projection; this enum
/// tells the settings directory whether a family has a safe editor, belongs
/// to a separate first-class module, or must stay with the client/organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficialSettingDisposition {
    /// A typed user-level value edited in the parameter form and projected by
    /// the normal supplier transaction.
    Direct,
    /// A user-level resource with its own contract and transaction. It must
    /// not be flattened into the parameter form.
    SeparateModule,
    /// A project, local, managed, credential, runtime, or not-yet-modelled
    /// structure that the application intentionally preserves without writing.
    PreserveOnly,
}

/// One visible official-setting family in the settings directory. This is the
/// public coverage map: every entry states its real write boundary instead of
/// letting an unlisted official key look accidentally unsupported.
#[derive(Debug, Clone, Copy)]
pub struct OfficialSettingEntry {
    pub title: &'static str,
    pub path: &'static str,
    pub related_paths: &'static [&'static str],
    pub disposition: OfficialSettingDisposition,
    pub detail: &'static str,
}

/// The value shape adapters and editor controls must preserve for a managed
/// key. The app does not infer a type from a client file at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingValueType {
    Bool,
    String,
    Secret,
    PositiveInteger,
    StringArray,
}

/// What a provider projection does when the current provider has no value for
/// one of its own keys. This is deliberately explicit so a previous provider
/// cannot leak a routing or model field into the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAbsentAction {
    Remove,
}

/// Rendering metadata for a common-settings editor control. Provider and host
/// settings have no common-settings control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingControl {
    None,
    Toggle,
    Choice { presentation: ChoiceControl },
}

/// One strongly typed entry in the ownership directory. `setting_spec` and
/// `setting_specs` are the only APIs consumers should use for ownership,
/// value constraints, legacy-migration baselines, UI metadata, and provider
/// cleanup decisions.
#[derive(Debug, Clone)]
pub struct SettingSpec {
    pub app: AppKind,
    pub key: &'static str,
    pub owner: SettingOwner,
    pub value_type: SettingValueType,
    pub allowed_values: &'static [ChoiceOption],
    pub control: SettingControl,
    /// Baseline emitted by the pre-automatic-value contract, used only for a
    /// one-time data migration. Runtime projection never treats it as a
    /// client default. Provider and host settings have no baseline here.
    pub legacy_default: Option<ConfigValue>,
    pub label: Option<&'static str>,
    pub group: Option<&'static str>,
    pub provider_absent_action: Option<ProviderAbsentAction>,
}

#[derive(Debug, Clone, Copy)]
struct ProviderSettingSpec {
    app: AppKind,
    key: &'static str,
    value_type: SettingValueType,
}

const PROVIDER_SETTINGS: &[ProviderSettingSpec] = &[
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: "model",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: "model_provider",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: "openai_base_url",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: "experimental_bearer_token",
        value_type: SettingValueType::Secret,
    },
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: "model_context_window",
        value_type: SettingValueType::PositiveInteger,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "model",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "availableModels",
        value_type: SettingValueType::StringArray,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_BASE_URL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_AUTH_TOKEN",
        value_type: SettingValueType::Secret,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_MODEL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
        value_type: SettingValueType::String,
    },
    // Deprecated by Claude, but still actively removed by a provider
    // projection so it cannot survive a switch as a hidden model mapping.
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_SMALL_FAST_MODEL",
        value_type: SettingValueType::String,
    },
];

/// One boolean general setting from the client's official configuration
/// reference. `legacy_default` is the old application contract's emitted
/// baseline and is used only while upgrading stored application state.
#[derive(Debug, Clone, Copy)]
pub struct ToggleSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub legacy_default: bool,
    pub group: &'static str,
}

/// One selectable value of a multi-detent general setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceOption {
    pub value: &'static str,
    pub label: &'static str,
}

/// How the settings page renders a choice: the reasoning-effort slider keeps
/// its dedicated slider control; every other choice renders as segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceControl {
    Slider,
    Segment,
}

/// One multi-value general setting from the client's official configuration
/// reference. `legacy_default` is the old application contract's emitted
/// baseline and is used only while upgrading stored application state.
#[derive(Debug, Clone, Copy)]
pub struct ChoiceSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub control: ChoiceControl,
    pub legacy_default: &'static str,
    pub options: &'static [ChoiceOption],
}

/// The official general-config toggles offered on the settings page. Keys
/// must stay inside the owned-key tables above.
pub const CODEX_TOGGLES: &[ToggleSpec] = &[
    ToggleSpec {
        key: "hide_agent_reasoning",
        label: "在界面中隐藏推理摘要",
        legacy_default: false,
        group: "模型行为",
    },
    ToggleSpec {
        key: "show_raw_agent_reasoning",
        label: "显示模型的原始推理内容",
        legacy_default: false,
        group: "模型行为",
    },
    ToggleSpec {
        key: "tui.animations",
        label: "终端动画（欢迎页与加载动效）",
        legacy_default: true,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.show_tooltips",
        label: "欢迎页功能引导提示",
        legacy_default: true,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.notifications",
        label: "终端通知（回合结束时）",
        legacy_default: false,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.raw_output_mode",
        label: "原始滚动模式（不切换交替屏幕）",
        legacy_default: false,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.vim_mode_default",
        label: "默认启用 Vim 输入模式",
        legacy_default: false,
        group: "终端界面",
    },
    ToggleSpec {
        key: "disable_paste_burst",
        label: "关闭多行粘贴突发检测",
        legacy_default: false,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tools.view_image",
        label: "启用本地图片查看工具",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.memories",
        label: "启用 Memories 跨会话记忆",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.prevent_idle_sleep",
        label: "会话运行期间阻止系统休眠",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "check_for_update_on_startup",
        label: "启动时检查更新",
        legacy_default: true,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "allow_login_shell",
        label: "允许登录 Shell 环境",
        legacy_default: false,
        group: "安全与审批",
    },
    ToggleSpec {
        key: "sandbox_workspace_write.network_access",
        label: "工作区沙箱允许网络访问",
        legacy_default: false,
        group: "安全与审批",
    },
    ToggleSpec {
        key: "sandbox_workspace_write.exclude_tmpdir_env_var",
        label: "工作区沙箱忽略 TMPDIR 环境变量",
        legacy_default: false,
        group: "安全与审批",
    },
    ToggleSpec {
        key: "sandbox_workspace_write.exclude_slash_tmp",
        label: "工作区沙箱不映射 /tmp",
        legacy_default: false,
        group: "安全与审批",
    },
    ToggleSpec {
        key: "windows.sandbox_private_desktop",
        label: "Windows 沙箱使用私有桌面",
        legacy_default: false,
        group: "安全与审批",
    },
    ToggleSpec {
        key: "feedback.enabled",
        label: "允许提交产品反馈",
        legacy_default: true,
        group: "隐私与数据",
    },
    ToggleSpec {
        key: "analytics.enabled",
        label: "允许使用分析数据",
        legacy_default: true,
        group: "隐私与数据",
    },
    ToggleSpec {
        key: "tui.alternate_screen",
        label: "使用终端交替屏幕",
        legacy_default: true,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.resume_cwd",
        label: "恢复会话时沿用工作目录",
        legacy_default: true,
        group: "终端界面",
    },
    ToggleSpec {
        key: "features.apps",
        label: "启用 Apps",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.hooks",
        label: "启用 Hooks",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.shell_tool",
        label: "启用 Shell 工具",
        legacy_default: true,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.enable_request_compression",
        label: "启用请求压缩",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.skill_mcp_dependency_install",
        label: "允许 Skill 安装 MCP 依赖",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.fast_mode",
        label: "启用快速模式",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.shell_snapshot",
        label: "启用 Shell 快照",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.unified_exec",
        label: "启用统一执行器",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.multi_agent",
        label: "启用多智能体协作",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.goals",
        label: "启用目标管理",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.remote_plugin",
        label: "启用远程插件",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.personality",
        label: "启用助手个性设置",
        legacy_default: true,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "agents.enabled",
        label: "启用多智能体执行",
        legacy_default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "agents.allow_interrupt",
        label: "允许中断子智能体",
        legacy_default: true,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "memories.generate_memories",
        label: "自动生成记忆",
        legacy_default: false,
        group: "隐私与数据",
    },
    ToggleSpec {
        key: "memories.use_memories",
        label: "在会话中使用记忆",
        legacy_default: false,
        group: "隐私与数据",
    },
    ToggleSpec {
        key: "memories.disable_on_external_context",
        label: "外部上下文时禁用记忆",
        legacy_default: true,
        group: "隐私与数据",
    },
];

pub const CLAUDE_TOGGLES: &[ToggleSpec] = &[
    ToggleSpec {
        key: "alwaysThinkingEnabled",
        label: "默认开启扩展思考",
        legacy_default: false,
        group: "模型行为",
    },
    ToggleSpec {
        key: "autoCompactEnabled",
        label: "上下文自动压缩",
        legacy_default: true,
        group: "模型行为",
    },
    ToggleSpec {
        key: "showThinkingSummaries",
        label: "思考过程摘要",
        legacy_default: true,
        group: "模型行为",
    },
    ToggleSpec {
        key: "spinnerTipsEnabled",
        label: "加载动画提示语",
        legacy_default: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "autoScrollEnabled",
        label: "输出自动滚动",
        legacy_default: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "emojiCompletionEnabled",
        label: "输入框表情补全",
        legacy_default: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "promptSuggestionEnabled",
        label: "提示词建议",
        legacy_default: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "showTurnDuration",
        label: "显示每轮回复耗时",
        legacy_default: false,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "syntaxHighlightingDisabled",
        label: "关闭输出语法高亮",
        legacy_default: false,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "terminalProgressBarEnabled",
        label: "终端底部进度条",
        legacy_default: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "fileCheckpointingEnabled",
        label: "文件检查点（对话内回滚）",
        legacy_default: true,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "respectGitignore",
        label: "文件选择遵守 .gitignore 规则",
        legacy_default: true,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "includeGitInstructions",
        label: "注入内置 Git 使用指南",
        legacy_default: true,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "autoMemoryEnabled",
        label: "自动记忆",
        legacy_default: true,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "ultracode",
        label: "Ultracode 动态工作流",
        legacy_default: false,
        group: "模型行为",
    },
];

pub const CODEX_CHOICES: &[ChoiceSpec] = &[
    ChoiceSpec {
        key: "model_reasoning_effort",
        label: "推理强度",
        group: "模型行为",
        control: ChoiceControl::Slider,
        legacy_default: "medium",
        options: &[
            ChoiceOption {
                value: "minimal",
                label: "极低",
            },
            ChoiceOption {
                value: "low",
                label: "低",
            },
            ChoiceOption {
                value: "medium",
                label: "中",
            },
            ChoiceOption {
                value: "high",
                label: "高",
            },
            ChoiceOption {
                value: "xhigh",
                label: "极高",
            },
        ],
    },
    ChoiceSpec {
        key: "plan_mode_reasoning_effort",
        label: "计划模式推理强度",
        group: "模型行为",
        control: ChoiceControl::Slider,
        legacy_default: "medium",
        options: &[
            ChoiceOption {
                value: "none",
                label: "无",
            },
            ChoiceOption {
                value: "minimal",
                label: "极低",
            },
            ChoiceOption {
                value: "low",
                label: "低",
            },
            ChoiceOption {
                value: "medium",
                label: "中",
            },
            ChoiceOption {
                value: "high",
                label: "高",
            },
            ChoiceOption {
                value: "xhigh",
                label: "极高",
            },
        ],
    },
    ChoiceSpec {
        key: "model_reasoning_summary",
        label: "推理摘要",
        group: "模型行为",
        control: ChoiceControl::Segment,
        legacy_default: "auto",
        options: &[
            ChoiceOption {
                value: "auto",
                label: "自动",
            },
            ChoiceOption {
                value: "concise",
                label: "简要",
            },
            ChoiceOption {
                value: "detailed",
                label: "详细",
            },
            ChoiceOption {
                value: "none",
                label: "关闭",
            },
        ],
    },
    ChoiceSpec {
        key: "model_verbosity",
        label: "回复详细度",
        group: "模型行为",
        control: ChoiceControl::Segment,
        legacy_default: "medium",
        options: &[
            ChoiceOption {
                value: "low",
                label: "简洁",
            },
            ChoiceOption {
                value: "medium",
                label: "标准",
            },
            ChoiceOption {
                value: "high",
                label: "详细",
            },
        ],
    },
    ChoiceSpec {
        key: "personality",
        label: "助手个性",
        group: "模型行为",
        control: ChoiceControl::Segment,
        legacy_default: "friendly",
        options: &[
            ChoiceOption {
                value: "none",
                label: "中性",
            },
            ChoiceOption {
                value: "friendly",
                label: "友好",
            },
            ChoiceOption {
                value: "pragmatic",
                label: "务实",
            },
        ],
    },
    ChoiceSpec {
        key: "web_search",
        label: "网页搜索",
        group: "模型行为",
        control: ChoiceControl::Segment,
        legacy_default: "disabled",
        options: &[
            ChoiceOption {
                value: "disabled",
                label: "禁用",
            },
            ChoiceOption {
                value: "cached",
                label: "仅缓存",
            },
            ChoiceOption {
                value: "indexed",
                label: "索引",
            },
            ChoiceOption {
                value: "live",
                label: "实时",
            },
        ],
    },
    ChoiceSpec {
        key: "sandbox_mode",
        label: "沙箱模式",
        group: "安全与审批",
        control: ChoiceControl::Segment,
        legacy_default: "read-only",
        options: &[
            ChoiceOption {
                value: "read-only",
                label: "只读",
            },
            ChoiceOption {
                value: "workspace-write",
                label: "工作区可写",
            },
            ChoiceOption {
                value: "danger-full-access",
                label: "完全访问",
            },
        ],
    },
    ChoiceSpec {
        key: "approval_policy",
        label: "批准策略",
        group: "安全与审批",
        control: ChoiceControl::Segment,
        legacy_default: "untrusted",
        options: &[
            ChoiceOption {
                value: "untrusted",
                label: "仅信任白名单代码",
            },
            ChoiceOption {
                value: "on-request",
                label: "按请求",
            },
            ChoiceOption {
                value: "never",
                label: "从不",
            },
        ],
    },
    ChoiceSpec {
        key: "approvals_reviewer",
        label: "审批复核方式",
        group: "安全与审批",
        control: ChoiceControl::Segment,
        legacy_default: "user",
        options: &[
            ChoiceOption {
                value: "user",
                label: "用户",
            },
            ChoiceOption {
                value: "auto_review",
                label: "自动复核",
            },
        ],
    },
    ChoiceSpec {
        key: "windows.sandbox",
        label: "Windows 沙箱权限",
        group: "安全与审批",
        control: ChoiceControl::Segment,
        legacy_default: "unelevated",
        options: &[
            ChoiceOption {
                value: "unelevated",
                label: "非提升",
            },
            ChoiceOption {
                value: "elevated",
                label: "提升权限",
            },
        ],
    },
    ChoiceSpec {
        key: "history.persistence",
        label: "会话历史",
        group: "隐私与数据",
        control: ChoiceControl::Segment,
        legacy_default: "save-all",
        options: &[
            ChoiceOption {
                value: "save-all",
                label: "保存全部",
            },
            ChoiceOption {
                value: "none",
                label: "不保存",
            },
        ],
    },
    ChoiceSpec {
        key: "file_opener",
        label: "文件打开方式",
        group: "工具与功能",
        control: ChoiceControl::Segment,
        legacy_default: "vscode",
        options: &[
            ChoiceOption {
                value: "vscode",
                label: "VS Code",
            },
            ChoiceOption {
                value: "vscode-insiders",
                label: "VS Code Insiders",
            },
            ChoiceOption {
                value: "windsurf",
                label: "Windsurf",
            },
            ChoiceOption {
                value: "cursor",
                label: "Cursor",
            },
            ChoiceOption {
                value: "none",
                label: "不打开",
            },
        ],
    },
];

pub const CLAUDE_CHOICES: &[ChoiceSpec] = &[
    ChoiceSpec {
        key: "effortLevel",
        label: "推理强度",
        group: "模型行为",
        control: ChoiceControl::Slider,
        legacy_default: "high",
        options: &[
            ChoiceOption {
                value: "low",
                label: "低",
            },
            ChoiceOption {
                value: "medium",
                label: "中",
            },
            ChoiceOption {
                value: "high",
                label: "高",
            },
            ChoiceOption {
                value: "xhigh",
                label: "极高",
            },
        ],
    },
    ChoiceSpec {
        key: "outputStyle",
        label: "输出风格",
        group: "模型行为",
        control: ChoiceControl::Segment,
        legacy_default: "Explanatory",
        options: &[
            ChoiceOption {
                value: "Proactive",
                label: "主动",
            },
            ChoiceOption {
                value: "Concise",
                label: "简洁",
            },
            ChoiceOption {
                value: "Explanatory",
                label: "讲解",
            },
            ChoiceOption {
                value: "Learning",
                label: "学习",
            },
        ],
    },
    ChoiceSpec {
        key: "preferredNotifChannel",
        label: "通知渠道",
        group: "界面与交互",
        control: ChoiceControl::Segment,
        legacy_default: "auto",
        options: &[
            ChoiceOption {
                value: "auto",
                label: "自动",
            },
            ChoiceOption {
                value: "terminal_bell",
                label: "终端铃声",
            },
            ChoiceOption {
                value: "iterm2",
                label: "iTerm2",
            },
            ChoiceOption {
                value: "iterm2_with_bell",
                label: "iTerm2 + 铃声",
            },
            ChoiceOption {
                value: "kitty",
                label: "Kitty",
            },
            ChoiceOption {
                value: "ghostty",
                label: "Ghostty",
            },
            ChoiceOption {
                value: "notifications_disabled",
                label: "关闭通知",
            },
        ],
    },
];

/// Official user-level configuration families that require a dedicated
/// contract, or are intentionally preserved because their scope is not the
/// user's global preferences. The parameter form receives only `Direct`
/// entries derived from `setting_specs`; this table prevents the rest of the
/// official surface from silently becoming an accidental "unknown key".
const CODEX_DIRECTORY_FAMILIES: &[OfficialSettingEntry] = &[
    OfficialSettingEntry {
        title: "全局指令",
        path: "$CODEX_HOME/AGENTS.md",
        related_paths: &[],
        disposition: OfficialSettingDisposition::SeparateModule,
        detail: "在“官方设置目录”中通过独立文档事务管理。",
    },
    OfficialSettingEntry {
        title: "自定义模型提供商",
        path: "model_providers.<id>",
        related_paths: &[],
        disposition: OfficialSettingDisposition::SeparateModule,
        detail: "由供应商档案与切换事务拥有，不能与通用参数重复写入。",
    },
    OfficialSettingEntry {
        title: "MCP 服务器",
        path: "mcp_servers.<id>",
        related_paths: &[],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "服务器、OAuth、工具授权与环境映射是结构化资源；当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "Hooks、Skills 与插件",
        path: "hooks",
        related_paths: &["skills.config", "plugins.<id>", "apps.<id>"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "命令、路径与事件规则需独立契约；当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "权限配置与高级沙箱",
        path: "permissions.<name>",
        related_paths: &["sandbox_workspace_write", "default_permissions"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "权限配置档、可写路径与网络规则不能压扁为普通开关；当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "Shell 环境与网络代理",
        path: "shell_environment_policy",
        related_paths: &["features.network_proxy"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "环境过滤、注入和域名规则具有结构与凭据边界；当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "TUI 键位与布局",
        path: "tui.keymap",
        related_paths: &["tui.status_line", "tui.terminal_title"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "有序布局和按上下文键位映射需要专用编辑器；当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "运行限额与终端超时",
        path: "model_auto_compact_token_limit",
        related_paths: &[
            "model_auto_compact_token_limit_scope",
            "history.max_bytes",
            "tool_output_token_limit",
            "background_terminal_max_timeout",
            "agents.max_concurrent_agents",
            "memories.*_limit",
        ],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "数值限额与保留策略需在同一资源预算契约中校验；当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "通知规则与终端状态",
        path: "tui.notification_method",
        related_paths: &["tui.notification_conditions", "tui.status_line"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "通知方法和条件为关联结构；普通偏好之外的部分当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "项目配置与信任状态",
        path: ".codex/config.toml",
        related_paths: &["projects.<path>.trust_level"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "属于具体项目，不作为跨项目用户基座写入。",
    },
    OfficialSettingEntry {
        title: "受管要求、登录与运行状态",
        path: "requirements.toml",
        related_paths: &["auth.json", "SQLite / 会话状态"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail:
            "属于组织策略与运行态；通用设置只观测不写入。auth.json 仅在用户发起官方登录或重新登录时由专用登录流程写入，其余时间只读观测。",
    },
];

const CLAUDE_DIRECTORY_FAMILIES: &[OfficialSettingEntry] = &[
    OfficialSettingEntry {
        title: "全局指令",
        path: "~/.claude/CLAUDE.md",
        related_paths: &[],
        disposition: OfficialSettingDisposition::SeparateModule,
        detail: "在“官方设置目录”中通过独立文档事务管理。",
    },
    OfficialSettingEntry {
        title: "权限规则",
        path: "permissions",
        related_paths: &[],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "allow / ask / deny 规则具有合并与优先级语义；当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "Hooks",
        path: "hooks",
        related_paths: &[],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "事件、匹配器和命令构成规则集合；当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "环境变量",
        path: "env",
        related_paths: &[],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail:
            "供应商拥有的 ANTHROPIC 路由字段与其他环境变量不能混为一个表单；当前保留未声明字段。",
    },
    OfficialSettingEntry {
        title: "模型映射与选择器",
        path: "modelOverrides",
        related_paths: &["modelSettings", "modelPicker", "fallbackModel"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "模型映射和回退链是有序结构，不能与供应商主模型形成双重所有权。",
    },
    OfficialSettingEntry {
        title: "按模型推理强度",
        path: "modelSettings.<model>.effortLevel",
        related_paths: &["modelSettings.<model>.fastMode"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "Claude 按模型保存的推理偏好和快速模式属于模型映射；全局 effortLevel 与 Ultracode 由基础参数直接管理。",
    },
    OfficialSettingEntry {
        title: "自定义输出风格",
        path: "outputStyles.<name>",
        related_paths: &["outputStyle"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "内置输出风格可直接选择；自定义风格是命名文档资源，当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "MCP 与插件",
        path: "~/.claude.json",
        related_paths: &[".mcp.json", "enabledPlugins", "extraKnownMarketplaces"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "MCP、插件市场和登录所在文件具有独立作用域；当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "语音与外部凭据脚本",
        path: "voice",
        related_paths: &["apiKeyHelper", "otelHeadersHelper"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "语音对象和会执行外部命令的凭据/遥测脚本需要独立契约；当前保留但不写入。",
    },
    OfficialSettingEntry {
        title: "项目、本地与受管配置",
        path: ".claude/settings.json",
        related_paths: &[".claude/settings.local.json", "managed-settings.json"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail: "分别属于仓库、项目个人例外和组织策略，不作为全局设置写入。",
    },
    OfficialSettingEntry {
        title: "官方登录与运行状态",
        path: "~/.claude/.credentials.json",
        related_paths: &["~/.claude.json"],
        disposition: OfficialSettingDisposition::PreserveOnly,
        detail:
            "官方身份、项目信任和运行态不由应用复制或导入为供应商档案；.credentials.json 仅在用户发起官方登录或重新登录时由专用登录流程写入，其余时间只读观测。",
    },
];

/// The common toggle entries for one client. They are a projection of the
/// ownership directory, not a second owned-key list.
pub fn common_toggles(app: AppKind) -> &'static [ToggleSpec] {
    match app {
        AppKind::Codex => CODEX_TOGGLES,
        AppKind::Claude => CLAUDE_TOGGLES,
    }
}

/// The common choice entries for one client. Values are the authority the
/// common-settings validator checks string entries against.
pub fn common_choices(app: AppKind) -> &'static [ChoiceSpec] {
    match app {
        AppKind::Codex => CODEX_CHOICES,
        AppKind::Claude => CLAUDE_CHOICES,
    }
}

/// Section order on the settings page, the single owner of grouping.
pub fn common_groups(app: AppKind) -> &'static [&'static str] {
    match app {
        AppKind::Codex => &[
            "模型行为",
            "安全与审批",
            "隐私与数据",
            "终端界面",
            "工具与功能",
        ],
        AppKind::Claude => &["模型行为", "界面与交互", "文件与 Git"],
    }
}

/// Complete official-settings coverage map for one client.
///
/// Direct parameter entries are derived from the same ownership directory
/// that validates and projects them. Structured, project-scoped and managed
/// families are listed alongside them with their truthful boundary, so the UI
/// never implies that a preserved resource is an editable scalar preference.
pub fn official_setting_directory(app: AppKind) -> Vec<OfficialSettingEntry> {
    let mut entries: Vec<OfficialSettingEntry> = setting_specs(app)
        .into_iter()
        .filter(|spec| spec.owner == SettingOwner::Common)
        .map(|spec| OfficialSettingEntry {
            title: spec
                .label
                .expect("common setting must have a visible label"),
            path: spec.key,
            related_paths: &[],
            disposition: OfficialSettingDisposition::Direct,
            detail: "通过“基础参数”编辑，保存后随供应商重新应用写入用户级配置。",
        })
        .collect();
    entries.extend_from_slice(match app {
        AppKind::Codex => CODEX_DIRECTORY_FAMILIES,
        AppKind::Claude => CLAUDE_DIRECTORY_FAMILIES,
    });
    entries
}

/// The catalog spec for a choice key, if the key is one.
pub fn choice_spec(app: AppKind, key: &str) -> Option<&'static ChoiceSpec> {
    common_choices(app).iter().find(|spec| spec.key == key)
}

/// The catalog spec for a toggle key, if the key is one.
pub fn toggle_spec(app: AppKind, key: &str) -> Option<&'static ToggleSpec> {
    common_toggles(app).iter().find(|spec| spec.key == key)
}

/// Returns every managed spec for one client. This is the single typed
/// directory exposed to adapters, validation, and the editor. Host keys do
/// not appear because the host namespace is intentionally open-ended.
pub fn setting_specs(app: AppKind) -> Vec<SettingSpec> {
    let mut specs = Vec::new();
    specs.extend(
        PROVIDER_SETTINGS
            .iter()
            .filter(|spec| spec.app == app)
            .map(|spec| SettingSpec {
                app,
                key: spec.key,
                owner: SettingOwner::Provider,
                value_type: spec.value_type,
                allowed_values: &[],
                control: SettingControl::None,
                legacy_default: None,
                label: None,
                group: None,
                provider_absent_action: Some(ProviderAbsentAction::Remove),
            }),
    );
    specs.extend(common_toggles(app).iter().map(|spec| SettingSpec {
        app,
        key: spec.key,
        owner: SettingOwner::Common,
        value_type: SettingValueType::Bool,
        allowed_values: &[],
        control: SettingControl::Toggle,
        legacy_default: Some(ConfigValue::Bool(spec.legacy_default)),
        label: Some(spec.label),
        group: Some(spec.group),
        provider_absent_action: None,
    }));
    specs.extend(common_choices(app).iter().map(|spec| SettingSpec {
        app,
        key: spec.key,
        owner: SettingOwner::Common,
        value_type: SettingValueType::String,
        allowed_values: spec.options,
        control: SettingControl::Choice {
            presentation: spec.control,
        },
        legacy_default: Some(ConfigValue::Str(spec.legacy_default.to_string())),
        label: Some(spec.label),
        group: Some(spec.group),
        provider_absent_action: None,
    }));
    specs
}

/// Looks up one client configuration key in the ownership directory. An
/// unlisted key is explicitly host-owned, rather than being an implicit
/// application fallback.
pub fn setting_spec(app: AppKind, key: &str) -> Option<SettingSpec> {
    setting_specs(app).into_iter().find(|spec| spec.key == key)
}

/// Returns the complete ownership decision for a key. This is a lightweight
/// form for callers that do not need editor metadata.
pub fn owner_for(app: AppKind, key: &str) -> SettingOwner {
    setting_spec(app, key)
        .map(|spec| spec.owner)
        .unwrap_or(SettingOwner::Host)
}

/// Returns the provider cleanup action for a provider-owned key, if any.
pub fn provider_absent_action(app: AppKind, key: &str) -> Option<ProviderAbsentAction> {
    setting_spec(app, key)
        .filter(|spec| spec.owner == SettingOwner::Provider)
        .and_then(|spec| spec.provider_absent_action)
}

/// Compatibility-free semantic spelling for adapter collectors: a key is
/// managed when the directory says it is common or provider-owned.
pub fn is_owned(app: AppKind, key: &str) -> bool {
    owner_for(app, key) != SettingOwner::Host
}

/// Whether a key belongs to the selected provider profile rather than the
/// common settings. The client argument is required: the same spelling can
/// have different ownership in different configuration formats.
pub fn is_provider_owned(app: AppKind, key: &str) -> bool {
    owner_for(app, key) == SettingOwner::Provider
}

/// The complete automatic common-settings intent for one client, built from
/// this directory. A fresh installation and every "恢复默认值" action leave
/// each client key to the host and active model rather than writing a guessed
/// value into the real configuration file.
pub fn default_common_settings(app: AppKind) -> CommonSettings {
    let mut settings = BTreeMap::new();
    for spec in setting_specs(app)
        .into_iter()
        .filter(|spec| spec.owner == SettingOwner::Common)
    {
        settings.insert(spec.key.to_string(), CommonSettingValue::Automatic);
    }
    CommonSettings { settings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_key_is_common_owned_in_the_directory() {
        for app in [AppKind::Codex, AppKind::Claude] {
            for toggle in common_toggles(app) {
                assert!(
                    is_owned(app, toggle.key),
                    "{} toggle key must be app-owned",
                    toggle.key
                );
                assert!(
                    owner_for(app, toggle.key) == SettingOwner::Common,
                    "{} toggle key must be common-owned",
                    toggle.key
                );
            }
            for choice in common_choices(app) {
                assert!(
                    is_owned(app, choice.key),
                    "{} choice key must be app-owned",
                    choice.key
                );
                assert!(
                    owner_for(app, choice.key) == SettingOwner::Common,
                    "{} choice key must be common-owned",
                    choice.key
                );
                assert!(
                    !choice.options.is_empty(),
                    "{} must offer at least one value",
                    choice.key
                );
            }
        }
    }

    #[test]
    fn official_directory_exposes_direct_and_preserved_boundaries() {
        for app in [AppKind::Codex, AppKind::Claude] {
            let directory = official_setting_directory(app);
            assert!(directory
                .iter()
                .any(|entry| { entry.disposition == OfficialSettingDisposition::Direct }));
            assert!(directory
                .iter()
                .any(|entry| { entry.disposition == OfficialSettingDisposition::PreserveOnly }));
            assert!(directory.iter().all(|entry| !entry.title.is_empty()
                && !entry.path.is_empty()
                && !entry.detail.is_empty()));
            for entry in directory
                .iter()
                .filter(|entry| entry.disposition == OfficialSettingDisposition::Direct)
            {
                assert_eq!(owner_for(app, entry.path), SettingOwner::Common);
            }
        }
    }

    #[test]
    fn every_catalog_group_is_declared_and_non_empty() {
        for app in [AppKind::Codex, AppKind::Claude] {
            let groups = common_groups(app);
            for group in groups {
                let group = *group;
                let members = common_toggles(app)
                    .iter()
                    .filter(|t| t.group == group)
                    .count()
                    + common_choices(app)
                        .iter()
                        .filter(|c| c.group == group)
                        .count();
                assert!(members > 0, "分组 {group} 必须至少有一个选项");
            }
            for spec in common_toggles(app) {
                assert!(
                    groups.contains(&spec.group),
                    "开关 {} 的分组 {} 未在 common_groups 声明",
                    spec.key,
                    spec.group
                );
            }
            for spec in common_choices(app) {
                assert!(
                    groups.contains(&spec.group),
                    "档位 {} 的分组 {} 未在 common_groups 声明",
                    spec.key,
                    spec.group
                );
            }
        }
    }

    #[test]
    fn every_legacy_baseline_is_one_of_the_offered_choice_values() {
        for app in [AppKind::Codex, AppKind::Claude] {
            for choice in common_choices(app) {
                assert!(
                    choice
                        .options
                        .iter()
                        .any(|option| option.value == choice.legacy_default),
                    "{} 的旧版基线必须是可选值之一",
                    choice.key
                );
            }
        }
    }

    #[test]
    fn automatic_common_settings_cover_exactly_the_catalog_keys() {
        for app in [AppKind::Codex, AppKind::Claude] {
            let defaults = default_common_settings(app);
            let catalog_keys: Vec<&str> = setting_specs(app)
                .into_iter()
                .filter(|spec| spec.owner == SettingOwner::Common)
                .map(|spec| spec.key)
                .collect();
            assert_eq!(defaults.settings.len(), catalog_keys.len());
            for key in catalog_keys {
                let value = defaults
                    .value(key)
                    .expect("every catalog key has an automatic value");
                assert!(matches!(value, CommonSettingValue::Automatic));
            }
        }
    }

    #[test]
    fn codex_top_level_routing_keys_are_owned() {
        assert!(is_owned(AppKind::Codex, "model"));
        assert!(is_owned(AppKind::Codex, "model_provider"));
        assert!(is_owned(AppKind::Codex, "openai_base_url"));
        assert!(is_owned(AppKind::Codex, "model_reasoning_effort"));
        assert!(is_owned(AppKind::Codex, "model_context_window"));
    }

    #[test]
    fn claude_model_tiers_and_available_models_are_owned() {
        assert!(is_owned(
            AppKind::Claude,
            "env.ANTHROPIC_DEFAULT_HAIKU_MODEL"
        ));
        assert!(is_owned(
            AppKind::Claude,
            "env.ANTHROPIC_DEFAULT_SONNET_MODEL"
        ));
        assert!(is_owned(
            AppKind::Claude,
            "env.ANTHROPIC_DEFAULT_OPUS_MODEL"
        ));
        assert!(is_owned(AppKind::Claude, "availableModels"));
    }

    #[test]
    fn claude_ultracode_is_an_independent_setting_not_an_effort_level() {
        let effort = choice_spec(AppKind::Claude, "effortLevel").expect("effort level");
        assert!(matches!(effort.control, ChoiceControl::Slider));
        assert!(effort
            .options
            .iter()
            .all(|option| option.value != "ultracode"));
        let ultracode = toggle_spec(AppKind::Claude, "ultracode").expect("ultracode toggle");
        assert_eq!(ultracode.group, "模型行为");
    }

    #[test]
    fn deprecated_claude_model_key_is_provider_owned_and_removed_when_absent() {
        assert_eq!(
            owner_for(AppKind::Claude, "env.ANTHROPIC_SMALL_FAST_MODEL"),
            SettingOwner::Provider
        );
        assert_eq!(
            provider_absent_action(AppKind::Claude, "env.ANTHROPIC_SMALL_FAST_MODEL"),
            Some(ProviderAbsentAction::Remove)
        );
    }

    #[test]
    fn host_keys_are_not_owned() {
        assert!(!is_owned(AppKind::Codex, "threads"));
        assert!(!is_owned(AppKind::Codex, "model_providers.openai.base_url"));
        assert!(!is_owned(AppKind::Claude, "permissions"));
        assert!(!is_owned(AppKind::Claude, "env.HTTP_PROXY"));
    }

    #[test]
    fn provider_keys_expose_types_and_a_cleanup_action_from_the_one_directory() {
        let model = setting_spec(AppKind::Codex, "model").expect("model spec");
        assert_eq!(model.owner, SettingOwner::Provider);
        assert_eq!(model.value_type, SettingValueType::String);
        assert_eq!(
            model.provider_absent_action,
            Some(ProviderAbsentAction::Remove)
        );
        assert!(is_provider_owned(
            AppKind::Codex,
            "experimental_bearer_token"
        ));
        assert_eq!(owner_for(AppKind::Codex, "threads"), SettingOwner::Host);
    }
}
