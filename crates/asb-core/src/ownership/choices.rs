use crate::ownership::spec::{ChoiceControl, ChoiceOption, ChoiceSpec, SettingOwner};
use crate::ownership::CODEX_WEB_SEARCH_KEY;

pub const CODEX_SUBAGENT_REASONING_EFFORT_KEY: &str = "agents.default_subagent_reasoning_effort";

/// The one real Codex reasoning-effort value domain. The same domain backs
/// every reasoning-effort setting and the catalog reasoning levels; values are
/// model-dependent upstream and are never inferred, only user-declared.
const CODEX_REASONING_EFFORT_OPTIONS: &[ChoiceOption] = &[
    ChoiceOption {
        value: "none",
        label: "ownership.option.reasoningEffort.none",
    },
    ChoiceOption {
        value: "minimal",
        label: "ownership.option.reasoningEffort.minimal",
    },
    ChoiceOption {
        value: "low",
        label: "ownership.option.reasoningEffort.low",
    },
    ChoiceOption {
        value: "medium",
        label: "ownership.option.reasoningEffort.medium",
    },
    ChoiceOption {
        value: "high",
        label: "ownership.option.reasoningEffort.high",
    },
    ChoiceOption {
        value: "xhigh",
        label: "ownership.option.reasoningEffort.xhigh",
    },
    ChoiceOption {
        value: "max",
        label: "ownership.option.reasoningEffort.max",
    },
    ChoiceOption {
        value: "ultra",
        label: "ownership.option.reasoningEffort.ultra",
    },
];

pub const CODEX_CHOICES: &[ChoiceSpec] = &[
    ChoiceSpec {
        key: "tui.notification_method", owner: SettingOwner::Client,
        label: "ownership.label.notificationMethod", group: "ownership.group.terminalUi",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "auto", label: "ownership.option.notificationMethod.auto" },
            ChoiceOption { value: "osc9", label: "ownership.option.notificationMethod.osc9" },
            ChoiceOption { value: "bel", label: "ownership.option.notificationMethod.bel" },
        ],
    },
    ChoiceSpec {
        key: "tui.notification_condition", owner: SettingOwner::Client,
        label: "ownership.label.notificationCondition", group: "ownership.group.terminalUi",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "unfocused", label: "ownership.option.notificationCondition.unfocused" },
            ChoiceOption { value: "always", label: "ownership.option.notificationCondition.always" },
        ],
    },
    ChoiceSpec {
        key: "tui.alternate_screen",
        owner: SettingOwner::Client,
        label: "ownership.label.tuiAlternate_screen",
        group: "ownership.group.terminalUi",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "auto", label: "ownership.option.tuiAlternate_screen.auto" },
            ChoiceOption { value: "always", label: "ownership.option.tuiAlternate_screen.always" },
            ChoiceOption { value: "never", label: "ownership.option.tuiAlternate_screen.never" },
        ],
    },
    ChoiceSpec {
        key: "tui.resume_cwd",
        owner: SettingOwner::Client,
        label: "ownership.label.tuiResume_cwd",
        group: "ownership.group.terminalUi",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "current", label: "ownership.option.tuiResume_cwd.current" },
            ChoiceOption { value: "session", label: "ownership.option.tuiResume_cwd.session" },
        ],
    },
    ChoiceSpec {
        key: "model_auto_compact_token_limit_scope",
        owner: SettingOwner::Provider,
        label: "ownership.label.autoCompactScope",
        group: "ownership.group.modelBehavior",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "total", label: "ownership.option.autoCompactScope.total" },
            ChoiceOption { value: "body_after_prefix", label: "ownership.option.autoCompactScope.body_after_prefix" },
        ],
    },
    ChoiceSpec {
        key: "model_reasoning_effort",
        owner: SettingOwner::Provider,
        label: "ownership.label.model_reasoning_effort",
        group: "ownership.group.modelBehavior",
        control: ChoiceControl::Slider,
        options: CODEX_REASONING_EFFORT_OPTIONS,
    },
    ChoiceSpec {
        key: CODEX_SUBAGENT_REASONING_EFFORT_KEY,
        owner: SettingOwner::Provider,
        label: "ownership.label.agentsDefault_subagent_reasoning_effort",
        group: "ownership.group.subagent",
        control: ChoiceControl::Slider,
        options: CODEX_REASONING_EFFORT_OPTIONS,
    },
    ChoiceSpec {
        key: "plan_mode_reasoning_effort",
        owner: SettingOwner::Provider,
        label: "ownership.label.plan_mode_reasoning_effort",
        group: "ownership.group.modelBehavior",
        control: ChoiceControl::Slider,
        options: CODEX_REASONING_EFFORT_OPTIONS,
    },
    ChoiceSpec {
        key: "model_reasoning_summary",
        owner: SettingOwner::Provider,
        label: "ownership.label.model_reasoning_summary",
        group: "ownership.group.modelBehavior",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "auto",
                label: "ownership.option.model_reasoning_summary.auto",
            },
            ChoiceOption {
                value: "concise",
                label: "ownership.option.model_reasoning_summary.concise",
            },
            ChoiceOption {
                value: "detailed",
                label: "ownership.option.model_reasoning_summary.detailed",
            },
            ChoiceOption {
                value: "none",
                label: "ownership.option.model_reasoning_summary.none",
            },
        ],
    },
    ChoiceSpec {
        key: "model_verbosity",
        owner: SettingOwner::Provider,
        label: "ownership.label.model_verbosity",
        group: "ownership.group.modelBehavior",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "low",
                label: "ownership.option.model_verbosity.low",
            },
            ChoiceOption {
                value: "medium",
                label: "ownership.option.model_verbosity.medium",
            },
            ChoiceOption {
                value: "high",
                label: "ownership.option.model_verbosity.high",
            },
        ],
    },
    ChoiceSpec {
        key: "personality",
        owner: SettingOwner::Provider,
        label: "ownership.label.personality",
        group: "ownership.group.modelBehavior",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "none",
                label: "ownership.option.personality.none",
            },
            ChoiceOption {
                value: "friendly",
                label: "ownership.option.personality.friendly",
            },
            ChoiceOption {
                value: "pragmatic",
                label: "ownership.option.personality.pragmatic",
            },
        ],
    },
    ChoiceSpec {
        key: CODEX_WEB_SEARCH_KEY,
        owner: SettingOwner::Provider,
        label: "ownership.label.webSearch",
        group: "ownership.group.modelBehavior",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "disabled",
                label: "ownership.option.webSearch.disabled",
            },
            ChoiceOption {
                value: "cached",
                label: "ownership.option.webSearch.cached",
            },
            ChoiceOption {
                value: "indexed",
                label: "ownership.option.webSearch.indexed",
            },
            ChoiceOption {
                value: "live",
                label: "ownership.option.webSearch.live",
            },
        ],
    },
    ChoiceSpec {
        key: "sandbox_mode",
        owner: SettingOwner::Client,
        label: "ownership.label.sandbox_mode",
        group: "ownership.group.safetyAndApprovals",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "read-only",
                label: "ownership.option.sandbox_mode.read-only",
            },
            ChoiceOption {
                value: "workspace-write",
                label: "ownership.option.sandbox_mode.workspace-write",
            },
            ChoiceOption {
                value: "danger-full-access",
                label: "ownership.option.sandbox_mode.danger-full-access",
            },
        ],
    },
    ChoiceSpec {
        key: "approvals_reviewer",
        owner: SettingOwner::Client,
        label: "ownership.label.approvals_reviewer",
        group: "ownership.group.safetyAndApprovals",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "user",
                label: "ownership.option.approvals_reviewer.user",
            },
            ChoiceOption {
                value: "auto_review",
                label: "ownership.option.approvals_reviewer.auto_review",
            },
        ],
    },
    ChoiceSpec {
        key: "windows.sandbox",
        owner: SettingOwner::Client,
        label: "ownership.label.windowsSandbox",
        group: "ownership.group.safetyAndApprovals",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "mxc",
                label: "ownership.option.windowsSandbox.mxc",
            },
            ChoiceOption {
                value: "unelevated",
                label: "ownership.option.windowsSandbox.unelevated",
            },
            ChoiceOption {
                value: "elevated",
                label: "ownership.option.windowsSandbox.elevated",
            },
        ],
    },
    ChoiceSpec {
        key: "history.persistence",
        owner: SettingOwner::Client,
        label: "ownership.label.historyPersistence",
        group: "ownership.group.privacyAndData",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "save-all",
                label: "ownership.option.historyPersistence.save-all",
            },
            ChoiceOption {
                value: "none",
                label: "ownership.option.historyPersistence.none",
            },
        ],
    },
    ChoiceSpec {
        key: "file_opener",
        owner: SettingOwner::Client,
        label: "ownership.label.file_opener",
        group: "ownership.group.toolsAndFeatures",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "vscode",
                label: "ownership.option.file_opener.vscode",
            },
            ChoiceOption {
                value: "vscode-insiders",
                label: "ownership.option.file_opener.vscode-insiders",
            },
            ChoiceOption {
                value: "windsurf",
                label: "ownership.option.file_opener.windsurf",
            },
            ChoiceOption {
                value: "cursor",
                label: "ownership.option.file_opener.cursor",
            },
            ChoiceOption {
                value: "none",
                label: "ownership.option.file_opener.none",
            },
        ],
    },
];
pub const CLAUDE_CHOICES: &[ChoiceSpec] = &[
    ChoiceSpec {
        key: "permissions.defaultMode", owner: SettingOwner::Client,
        label: "ownership.label.permissionMode", group: "ownership.group.safetyAndApprovals",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "default", label: "ownership.option.permissionMode.default" },
            ChoiceOption { value: "acceptEdits", label: "ownership.option.permissionMode.acceptEdits" },
            ChoiceOption { value: "plan", label: "ownership.option.permissionMode.plan" },
            ChoiceOption { value: "auto", label: "ownership.option.permissionMode.auto" },
            ChoiceOption { value: "dontAsk", label: "ownership.option.permissionMode.dontAsk" },
            ChoiceOption { value: "bypassPermissions", label: "ownership.option.permissionMode.bypassPermissions" },
        ],
    },
    ChoiceSpec {
        key: "editorMode", owner: SettingOwner::Client,
        label: "ownership.label.editorMode", group: "ownership.group.interfaceAndInteraction",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "normal", label: "ownership.option.editorMode.normal" },
            ChoiceOption { value: "vim", label: "ownership.option.editorMode.vim" },
        ],
    },
    ChoiceSpec {
        key: "defaultShell", owner: SettingOwner::Client,
        label: "ownership.label.defaultShell", group: "ownership.group.interfaceAndInteraction",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "bash", label: "ownership.option.defaultShell.bash" },
            ChoiceOption { value: "powershell", label: "ownership.option.defaultShell.powershell" },
        ],
    },
    ChoiceSpec {
        key: "autoUpdatesChannel", owner: SettingOwner::Client,
        label: "ownership.label.autoUpdatesChannel", group: "ownership.group.toolsAndFeatures",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "stable", label: "ownership.option.autoUpdatesChannel.stable" },
            ChoiceOption { value: "latest", label: "ownership.option.autoUpdatesChannel.latest" },
        ],
    },
    ChoiceSpec {
        key: "maxEffortLevel", owner: SettingOwner::Provider,
        label: "ownership.label.maxEffortLevel", group: "ownership.group.modelBehavior",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "low", label: "ownership.option.maxEffortLevel.low" },
            ChoiceOption { value: "medium", label: "ownership.option.maxEffortLevel.medium" },
            ChoiceOption { value: "high", label: "ownership.option.maxEffortLevel.high" },
            ChoiceOption { value: "xhigh", label: "ownership.option.maxEffortLevel.xhigh" },
            ChoiceOption { value: "max", label: "ownership.option.maxEffortLevel.max" },
        ],
    },
    ChoiceSpec {
        key: "promptCacheTtl", owner: SettingOwner::Provider,
        label: "ownership.label.promptCacheTtl", group: "ownership.group.modelBehavior",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "5m", label: "ownership.option.promptCacheTtl.5m" },
            ChoiceOption { value: "1h", label: "ownership.option.promptCacheTtl.1h" },
        ],
    },
    ChoiceSpec {
        key: "subagentPromptCacheTtl", owner: SettingOwner::Provider,
        label: "ownership.label.subagentPromptCacheTtl", group: "ownership.group.modelBehavior",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption { value: "5m", label: "ownership.option.subagentPromptCacheTtl.5m" },
            ChoiceOption { value: "1h", label: "ownership.option.subagentPromptCacheTtl.1h" },
        ],
    },
    ChoiceSpec {
        key: "outputStyle",
        owner: SettingOwner::Provider,
        label: "ownership.label.outputStyle",
        group: "ownership.group.modelBehavior",
        control: ChoiceControl::Editable,
        options: &[
            ChoiceOption {
                value: "Proactive",
                label: "ownership.option.outputStyle.Proactive",
            },
            ChoiceOption {
                value: "Concise",
                label: "ownership.option.outputStyle.Concise",
            },
            ChoiceOption {
                value: "Explanatory",
                label: "ownership.option.outputStyle.Explanatory",
            },
            ChoiceOption {
                value: "Learning",
                label: "ownership.option.outputStyle.Learning",
            },
        ],
    },
    ChoiceSpec {
        key: "preferredNotifChannel",
        owner: SettingOwner::Client,
        label: "ownership.label.preferredNotifChannel",
        group: "ownership.group.interfaceAndInteraction",
        control: ChoiceControl::Segment,
        options: &[
            ChoiceOption {
                value: "auto",
                label: "ownership.option.preferredNotifChannel.auto",
            },
            ChoiceOption {
                value: "terminal_bell",
                label: "ownership.option.preferredNotifChannel.terminal_bell",
            },
            ChoiceOption {
                value: "iterm2",
                label: "ownership.option.preferredNotifChannel.iterm2",
            },
            ChoiceOption {
                value: "iterm2_with_bell",
                label: "ownership.option.preferredNotifChannel.iterm2_with_bell",
            },
            ChoiceOption {
                value: "kitty",
                label: "ownership.option.preferredNotifChannel.kitty",
            },
            ChoiceOption {
                value: "ghostty",
                label: "ownership.option.preferredNotifChannel.ghostty",
            },
            ChoiceOption {
                value: "notifications_disabled",
                label: "ownership.option.preferredNotifChannel.notifications_disabled",
            },
        ],
    },
];
