//! Ownership directory: the single authority for which configuration keys
//! Agent Switchboard is allowed to read, patch, or write.
//!
//! Every key outside these sets is host-owned and must be preserved
//! byte-for-byte. Patches referencing a host-owned key are rejected at
//! validation time, never silently dropped.

use crate::contracts::{AppKind, CommonSettings, ConfigValue};
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
/// value constraints, defaults, UI metadata, and provider cleanup decisions.
#[derive(Debug, Clone)]
pub struct SettingSpec {
    pub app: AppKind,
    pub key: &'static str,
    pub owner: SettingOwner,
    pub value_type: SettingValueType,
    pub allowed_values: &'static [ChoiceOption],
    pub control: SettingControl,
    /// The directory-defined default for a common setting. Provider and host
    /// settings have no default here.
    pub default: Option<ConfigValue>,
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
/// reference. `default` is the value the client uses when the line is
/// absent; the editor always stores an explicit value.
#[derive(Debug, Clone, Copy)]
pub struct ToggleSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub default: bool,
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
/// reference. `default` is the value the client uses when the line is absent.
#[derive(Debug, Clone, Copy)]
pub struct ChoiceSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub control: ChoiceControl,
    pub default: &'static str,
    pub options: &'static [ChoiceOption],
}

/// The official general-config toggles offered on the settings page. Keys
/// must stay inside the owned-key tables above.
pub const CODEX_TOGGLES: &[ToggleSpec] = &[
    ToggleSpec {
        key: "hide_agent_reasoning",
        label: "在界面中隐藏推理摘要",
        default: false,
        group: "模型行为",
    },
    ToggleSpec {
        key: "show_raw_agent_reasoning",
        label: "显示模型的原始推理内容",
        default: false,
        group: "模型行为",
    },
    ToggleSpec {
        key: "disable_response_storage",
        label: "OpenAI 服务端不保存你的请求与响应",
        default: false,
        group: "隐私与数据",
    },
    ToggleSpec {
        key: "tui.animations",
        label: "终端动画（欢迎页与加载动效）",
        default: true,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.show_tooltips",
        label: "欢迎页功能引导提示",
        default: true,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.notifications",
        label: "终端通知（回合结束时）",
        default: false,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.raw_output_mode",
        label: "原始滚动模式（不切换交替屏幕）",
        default: false,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tui.vim_mode_default",
        label: "默认启用 Vim 输入模式",
        default: false,
        group: "终端界面",
    },
    ToggleSpec {
        key: "disable_paste_burst",
        label: "关闭多行粘贴突发检测",
        default: false,
        group: "终端界面",
    },
    ToggleSpec {
        key: "tools.view_image",
        label: "启用本地图片查看工具",
        default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.memories",
        label: "启用 Memories 跨会话记忆",
        default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "features.prevent_idle_sleep",
        label: "会话运行期间阻止系统休眠",
        default: false,
        group: "工具与功能",
    },
    ToggleSpec {
        key: "check_for_update_on_startup",
        label: "启动时检查更新",
        default: true,
        group: "工具与功能",
    },
];

pub const CLAUDE_TOGGLES: &[ToggleSpec] = &[
    ToggleSpec {
        key: "alwaysThinkingEnabled",
        label: "每次会话默认开启扩展思考",
        default: false,
        group: "模型行为",
    },
    ToggleSpec {
        key: "autoCompactEnabled",
        label: "上下文自动压缩",
        default: true,
        group: "模型行为",
    },
    ToggleSpec {
        key: "showThinkingSummaries",
        label: "思考过程摘要",
        default: true,
        group: "模型行为",
    },
    ToggleSpec {
        key: "spinnerTipsEnabled",
        label: "加载动画提示语",
        default: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "autoScrollEnabled",
        label: "输出自动滚动",
        default: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "emojiCompletionEnabled",
        label: "输入框表情补全",
        default: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "promptSuggestionEnabled",
        label: "提示词建议",
        default: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "showTurnDuration",
        label: "显示每轮回复耗时",
        default: false,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "syntaxHighlightingDisabled",
        label: "关闭输出语法高亮",
        default: false,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "terminalProgressBarEnabled",
        label: "终端底部进度条",
        default: true,
        group: "界面与交互",
    },
    ToggleSpec {
        key: "fileCheckpointingEnabled",
        label: "文件检查点（对话内回滚）",
        default: true,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "respectGitignore",
        label: "文件选择遵守 .gitignore 规则",
        default: true,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "includeGitInstructions",
        label: "注入内置 Git 使用指南",
        default: true,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "attribution.coAuthoredBy",
        label: "提交与 PR 添加 Claude 署名",
        default: true,
        group: "文件与 Git",
    },
    ToggleSpec {
        key: "autoMemoryEnabled",
        label: "自动记忆",
        default: true,
        group: "文件与 Git",
    },
];

pub const CODEX_CHOICES: &[ChoiceSpec] = &[
    ChoiceSpec {
        key: "model_reasoning_effort",
        label: "推理强度",
        group: "模型行为",
        control: ChoiceControl::Slider,
        default: "medium",
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
        default: "auto",
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
        default: "medium",
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
        default: "friendly",
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
        default: "disabled",
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
        default: "read-only",
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
        default: "untrusted",
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
        default: "save-all",
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
        default: "default",
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
        default: "auto",
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
                default: None,
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
        default: Some(ConfigValue::Bool(spec.default)),
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
        default: Some(ConfigValue::Str(spec.default.to_string())),
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

/// The complete default common settings for one client, built from this
/// directory. It is the value a fresh installation exposes before any save
/// and the target of every "恢复默认值" action.
pub fn default_common_settings(app: AppKind) -> CommonSettings {
    let mut settings = BTreeMap::new();
    for spec in setting_specs(app)
        .into_iter()
        .filter(|spec| spec.owner == SettingOwner::Common)
    {
        let default = spec
            .default
            .expect("every common setting declares a directory default");
        settings.insert(spec.key.to_string(), default);
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
    fn every_default_is_one_of_the_offered_choice_values() {
        for app in [AppKind::Codex, AppKind::Claude] {
            for choice in common_choices(app) {
                assert!(
                    choice
                        .options
                        .iter()
                        .any(|option| option.value == choice.default),
                    "{} 的默认值必须是可选值之一",
                    choice.key
                );
            }
        }
    }

    #[test]
    fn default_common_settings_cover_exactly_the_catalog_keys() {
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
                    .expect("every catalog key has a default value");
                if matches!(
                    setting_spec(app, key).expect("spec").control,
                    SettingControl::Toggle
                ) {
                    assert!(matches!(value, ConfigValue::Bool(_)));
                } else {
                    assert!(matches!(value, ConfigValue::Str(_)));
                }
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
        assert!(!is_provider_owned(
            AppKind::Codex,
            "disable_response_storage"
        ));
        assert_eq!(owner_for(AppKind::Codex, "threads"), SettingOwner::Host);
    }
}
