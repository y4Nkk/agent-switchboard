//! Ownership table: the single authority for which configuration keys
//! Agent Switchboard is allowed to read, patch, or write.
//!
//! Every key outside these sets is host-owned and must be preserved
//! byte-for-byte. Patches referencing a host-owned key are rejected at
//! validation time, never silently dropped.

use crate::contracts::AppKind;

/// Top-level keys owned by the app for each client.
pub const CODEX_OWNED_KEYS: &[&str] = &[
    "model",
    "model_provider",
    "model_reasoning_effort",
    "model_reasoning_summary",
    "model_verbosity",
    "model_context_window",
    "experimental_bearer_token",
    "disable_response_storage",
    "hide_agent_reasoning",
    "show_raw_agent_reasoning",
    "personality",
    "web_search",
    "sandbox_mode",
    "approval_policy",
    "history.persistence",
    "tui.animations",
    "tui.show_tooltips",
    "tui.notifications",
    "tui.raw_output_mode",
    "tui.vim_mode_default",
    "disable_paste_burst",
    "tools.view_image",
    "features.memories",
    "features.prevent_idle_sleep",
    "check_for_update_on_startup",
];

/// Claude Code keys owned by the app (dotted paths into settings.json).
pub const CLAUDE_OWNED_KEYS: &[&str] = &[
    "model",
    "availableModels",
    "env.ANTHROPIC_BASE_URL",
    "env.ANTHROPIC_AUTH_TOKEN",
    "env.ANTHROPIC_MODEL",
    "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
    "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
    "alwaysThinkingEnabled",
    "spinnerTipsEnabled",
    "attribution.coAuthoredBy",
    "autoCompactEnabled",
    "showThinkingSummaries",
    "outputStyle",
    "preferredNotifChannel",
    "autoScrollEnabled",
    "emojiCompletionEnabled",
    "promptSuggestionEnabled",
    "showTurnDuration",
    "syntaxHighlightingDisabled",
    "terminalProgressBarEnabled",
    "fileCheckpointingEnabled",
    "respectGitignore",
    "includeGitInstructions",
    "autoMemoryEnabled",
];

/// Keys the adapters own but that may only be set through a provider profile,
/// never through a general-config patch. Model routing follows the profile
/// being switched to; run-behavior preferences (effort, summary, verbosity)
/// are NOT here: they are general settings owned by the settings page.
pub const PROFILE_EXCLUSIVE_KEYS: &[&str] = &[
    "model",
    "model_provider",
    "model_context_window",
    "availableModels",
    "env.ANTHROPIC_BASE_URL",
    "env.ANTHROPIC_AUTH_TOKEN",
    "env.ANTHROPIC_MODEL",
    "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
    "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
];

/// Key prefixes owned by the app. Codex manages exactly one provider table,
/// `model_providers.asb`; every other `model_providers.*` table is host-owned.
pub const CODEX_OWNED_PREFIXES: &[&str] = &["model_providers.asb."];

/// One checkbox-able general setting from the client's official
/// configuration reference. `applied` is the value the checked line carries;
/// unchecking removes the line so the client default applies.
pub struct ToggleSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub line: &'static str,
    pub applied: bool,
    pub group: &'static str,
}

/// One selectable value of a multi-detent general setting.
pub struct ChoiceOption {
    pub value: &'static str,
    pub label: &'static str,
}

/// How the settings page renders a choice: the reasoning-effort slider keeps
/// its dedicated slider control; every other choice renders as segments.
pub enum ChoiceControl {
    Slider,
    Segment,
}

/// One multi-value general setting from the client's official configuration
/// reference. Selecting an option writes that line; selecting 默认 removes it.
pub struct ChoiceSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub control: ChoiceControl,
    pub options: &'static [ChoiceOption],
}

/// The official general-config toggles offered on the settings page. Keys
/// must stay inside the owned-key tables above.
pub const CODEX_TOGGLES: &[ToggleSpec] = &[
    ToggleSpec {
        key: "hide_agent_reasoning",
        label: "在界面中隐藏推理摘要",
        line: "hide_agent_reasoning = true",
        applied: true,
        group: "模型行为",
    },
    ToggleSpec {
        key: "show_raw_agent_reasoning",
        label: "显示模型的原始推理内容",
        line: "show_raw_agent_reasoning = true",
        applied: true,
        group: "模型行为",
    },
    ToggleSpec {
        key: "disable_response_storage",
        label: "勾选后 OpenAI 服务端不保存你的请求与响应",
        line: "disable_response_storage = true",
        applied: true,
        group: "隐私与数据",
    },
    ToggleSpec {
        key: "tui.animations",
        label: "关闭终端动画（欢迎页与加载动效）",
        line: "tui.animations = false",
        applied: false,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.show_tooltips",
        label: "关闭欢迎页功能引导提示",
        line: "tui.show_tooltips = false",
        applied: false,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.notifications",
        label: "开启终端通知（回合结束时）",
        line: "tui.notifications = true",
        applied: true,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.raw_output_mode",
        label: "开启原始滚动模式（不切换交替屏幕）",
        line: "tui.raw_output_mode = true",
        applied: true,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.vim_mode_default",
        label: "默认启用 Vim 输入模式",
        line: "tui.vim_mode_default = true",
        applied: true,
        group: "终端界面",
    },
    ToggleSpec {
        key: "disable_paste_burst",
        label: "关闭多行粘贴突发检测",
        line: "disable_paste_burst = true",
        applied: true,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tools.view_image",
        label: "启用本地图片查看工具",
        line: "tools.view_image = true",
        applied: true,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.memories",
        label: "启用 Memories 跨会话记忆",
        line: "features.memories = true",
        applied: true,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.prevent_idle_sleep",
        label: "会话运行期间阻止系统休眠",
        line: "features.prevent_idle_sleep = true",
        applied: true,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "check_for_update_on_startup",
        label: "关闭启动时检查更新",
        line: "check_for_update_on_startup = false",
        applied: false,
        group: "工具与功能",
    },
];

pub const CLAUDE_TOGGLES: &[ToggleSpec] = &[
    ToggleSpec {
        key: "alwaysThinkingEnabled",
        label: "每次会话默认开启扩展思考",
        line: "alwaysThinkingEnabled = true",
        applied: true,
        group: "模型行为",
    },
    ToggleSpec {
        key: "autoCompactEnabled",
        label: "关闭上下文自动压缩",
        line: "autoCompactEnabled = false",
        applied: false,
        group: "模型行为",
    },
    ToggleSpec {
        key: "showThinkingSummaries",
        label: "隐藏思考过程摘要",
        line: "showThinkingSummaries = false",
        applied: false,
        group: "模型行为",
    },
    ToggleSpec {
        key: "spinnerTipsEnabled",
        label: "关闭加载动画中的提示语",
        line: "spinnerTipsEnabled = false",
        applied: false,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "autoScrollEnabled",
        label: "关闭输出自动滚动",
        line: "autoScrollEnabled = false",
        applied: false,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "emojiCompletionEnabled",
        label: "关闭输入框表情补全",
        line: "emojiCompletionEnabled = false",
        applied: false,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "promptSuggestionEnabled",
        label: "关闭提示词建议",
        line: "promptSuggestionEnabled = false",
        applied: false,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "showTurnDuration",
        label: "显示每轮回复耗时",
        line: "showTurnDuration = true",
        applied: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "syntaxHighlightingDisabled",
        label: "关闭输出语法高亮",
        line: "syntaxHighlightingDisabled = true",
        applied: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "terminalProgressBarEnabled",
        label: "关闭终端底部进度条",
        line: "terminalProgressBarEnabled = false",
        applied: false,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "fileCheckpointingEnabled",
        label: "关闭文件检查点（放弃对话内回滚）",
        line: "fileCheckpointingEnabled = false",
        applied: false,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "respectGitignore",
        label: "文件选择忽略 .gitignore 规则",
        line: "respectGitignore = false",
        applied: false,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "includeGitInstructions",
        label: "不注入内置 Git 使用指南",
        line: "includeGitInstructions = false",
        applied: false,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "attribution.coAuthoredBy",
        label: "提交与 PR 不添加 Claude 署名",
        line: "attribution.coAuthoredBy = false",
        applied: false,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "autoMemoryEnabled",
        label: "关闭自动记忆",
        line: "autoMemoryEnabled = false",
        applied: false,
        group: "文件与 Git",
    },
];

pub const CODEX_CHOICES: &[ChoiceSpec] = &[
    ChoiceSpec {
        key: "model_reasoning_effort",
        label: "推理强度",
        group: "模型行为",
        control: ChoiceControl::Slider,
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
        key: "model_reasoning_summary",
        label: "推理摘要",
        group: "模型行为",
        control: ChoiceControl::Segment,
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
        key: "history.persistence",
        label: "会话历史",
        group: "隐私与数据",
        control: ChoiceControl::Segment,
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
];

pub const CLAUDE_CHOICES: &[ChoiceSpec] = &[
    ChoiceSpec {
        key: "outputStyle",
        label: "输出风格",
        group: "模型行为",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "default",
                label: "标准",
            },
            ChoiceOption {
                value: "explanatory",
                label: "讲解",
            },
            ChoiceOption {
                value: "learning",
                label: "学习",
            },
        ],
    },
    ChoiceSpec {
        key: "preferredNotifChannel",
        label: "通知渠道",
        group: "界面与交互",
        control: ChoiceControl::Segment,
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
                value: "notifications_disabled",
                label: "关闭通知",
            },
        ],
    },
];

/// The toggle catalog for one client; the settings UI consumes it instead of
/// redefining the key list.
pub fn common_toggles(app: AppKind) -> &'static [ToggleSpec] {
    match app {
        AppKind::Codex => CODEX_TOGGLES,
        AppKind::Claude => CLAUDE_TOGGLES,
    }
}

/// The multi-detent catalog for one client. Values are the authority the
/// common-patch validator checks string entries against.
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

/// The catalog spec for a choice key, if the key is one.
pub fn choice_spec(app: AppKind, key: &str) -> Option<&'static ChoiceSpec> {
    common_choices(app).iter().find(|spec| spec.key == key)
}

/// The catalog spec for a toggle key, if the key is one.
pub fn toggle_spec(app: AppKind, key: &str) -> Option<&'static ToggleSpec> {
    common_toggles(app).iter().find(|spec| spec.key == key)
}

/// Returns true when `key` is app-owned for `app`.
pub fn is_owned(app: AppKind, key: &str) -> bool {
    let (keys, prefixes) = match app {
        AppKind::Codex => (CODEX_OWNED_KEYS, CODEX_OWNED_PREFIXES),
        AppKind::Claude => (CLAUDE_OWNED_KEYS, &[] as &[&str]),
    };
    keys.contains(&key)
        || prefixes
            .iter()
            .any(|p| key.starts_with(p) || key == p.trim_end_matches('.'))
}

/// Returns true when `key` may only be set through a provider profile.
pub fn is_profile_exclusive(key: &str) -> bool {
    PROFILE_EXCLUSIVE_KEYS.contains(&key)
        || key == "model_providers.asb"
        || key.starts_with("model_providers.asb.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_key_is_owned_and_not_profile_exclusive() {
        for app in [AppKind::Codex, AppKind::Claude] {
            for toggle in common_toggles(app) {
                assert!(
                    is_owned(app, toggle.key),
                    "{} toggle key must be app-owned",
                    toggle.key
                );
                assert!(
                    !is_profile_exclusive(toggle.key),
                    "{} toggle key must not be profile-exclusive",
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
                    !is_profile_exclusive(choice.key),
                    "{} choice key must not be profile-exclusive",
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
    fn codex_top_level_and_managed_table_are_owned() {
        assert!(is_owned(AppKind::Codex, "model"));
        assert!(is_owned(AppKind::Codex, "model_provider"));
        assert!(is_owned(AppKind::Codex, "model_reasoning_effort"));
        assert!(is_owned(AppKind::Codex, "model_context_window"));
        assert!(is_owned(AppKind::Codex, "model_providers.asb"));
        assert!(is_owned(AppKind::Codex, "model_providers.asb.base_url"));
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
    fn deprecated_claude_model_key_is_not_owned() {
        assert!(!is_owned(AppKind::Claude, "env.ANTHROPIC_SMALL_FAST_MODEL"));
    }

    #[test]
    fn host_keys_are_not_owned() {
        assert!(!is_owned(AppKind::Codex, "threads"));
        assert!(!is_owned(AppKind::Codex, "model_providers.openai.base_url"));
        assert!(!is_owned(AppKind::Claude, "permissions"));
        assert!(!is_owned(AppKind::Claude, "env.HTTP_PROXY"));
    }

    #[test]
    fn profile_exclusive_keys_never_appear_in_general_patches() {
        assert!(is_profile_exclusive("model"));
        assert!(is_profile_exclusive("env.ANTHROPIC_BASE_URL"));
        assert!(is_profile_exclusive("model_providers.asb.base_url"));
        assert!(!is_profile_exclusive("disable_response_storage"));
    }
}
