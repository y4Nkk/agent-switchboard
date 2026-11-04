use super::spec::{SettingControl, SettingOwner};
use crate::contracts::{AppKind, MAX_EXACT_CONFIG_INTEGER};
use crate::claude_common::model_preferences::{COMPACT_WINDOW_MIN, COMPACT_WINDOW_MAX};

pub(super) struct InputSpec {
    pub app: AppKind,
    pub key: &'static str,
    pub owner: SettingOwner,
    pub label: &'static str,
    pub group: &'static str,
    pub control: SettingControl,
}

pub(super) const INPUTS: &[InputSpec] = &[
    InputSpec {
        app: AppKind::Codex, key: "approval_policy", owner: SettingOwner::Client,
        label: "ownership.label.approval_policy", group: "ownership.group.safetyAndApprovals",
        control: SettingControl::ApprovalPolicy,
    },
    InputSpec {
        app: AppKind::Codex, key: "tui.theme", owner: SettingOwner::Client,
        label: "ownership.label.tuiTheme", group: "ownership.group.terminalUi",
        control: SettingControl::Text,
    },
    InputSpec {
        app: AppKind::Codex, key: "model_post_turn_compact_threshold_percent", owner: SettingOwner::Provider,
        label: "ownership.label.postTurnCompact", group: "ownership.group.modelBehavior",
        control: SettingControl::Integer { min: 0, max: 100 },
    },
    InputSpec {
        app: AppKind::Claude, key: "permissions.allow", owner: SettingOwner::Client,
        label: "ownership.label.permissionsAllow", group: "ownership.group.safetyAndApprovals",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "permissions.ask", owner: SettingOwner::Client,
        label: "ownership.label.permissionsAsk", group: "ownership.group.safetyAndApprovals",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "permissions.deny", owner: SettingOwner::Client,
        label: "ownership.label.permissionsDeny", group: "ownership.group.safetyAndApprovals",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "permissions.additionalDirectories", owner: SettingOwner::Client,
        label: "ownership.label.additionalDirectories", group: "ownership.group.safetyAndApprovals",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "sandbox.excludedCommands", owner: SettingOwner::Client,
        label: "ownership.label.sandboxExcludedCommands", group: "ownership.group.sandbox",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "sandbox.filesystem.allowWrite", owner: SettingOwner::Client,
        label: "ownership.label.sandboxAllowWrite", group: "ownership.group.sandbox",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "sandbox.filesystem.denyWrite", owner: SettingOwner::Client,
        label: "ownership.label.sandboxDenyWrite", group: "ownership.group.sandbox",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "sandbox.filesystem.denyRead", owner: SettingOwner::Client,
        label: "ownership.label.sandboxDenyRead", group: "ownership.group.sandbox",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "sandbox.filesystem.allowRead", owner: SettingOwner::Client,
        label: "ownership.label.sandboxAllowRead", group: "ownership.group.sandbox",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "sandbox.network.allowedDomains", owner: SettingOwner::Client,
        label: "ownership.label.sandboxAllowedDomains", group: "ownership.group.sandbox",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "sandbox.network.deniedDomains", owner: SettingOwner::Client,
        label: "ownership.label.sandboxDeniedDomains", group: "ownership.group.sandbox",
        control: SettingControl::StringList,
    },
    InputSpec {
        app: AppKind::Claude, key: "cleanupPeriodDays", owner: SettingOwner::Client,
        label: "ownership.label.cleanupPeriodDays", group: "ownership.group.filesAndGit",
        control: SettingControl::Integer { min: 1, max: MAX_EXACT_CONFIG_INTEGER as u64 },
    },
    InputSpec {
        app: AppKind::Claude, key: "plansDirectory", owner: SettingOwner::Client,
        label: "ownership.label.plansDirectory", group: "ownership.group.filesAndGit",
        control: SettingControl::Text,
    },
    InputSpec {
        app: AppKind::Claude, key: "attribution.commit", owner: SettingOwner::Client,
        label: "ownership.label.attributionCommit", group: "ownership.group.filesAndGit",
        control: SettingControl::MultilineText,
    },
    InputSpec {
        app: AppKind::Claude, key: "attribution.pr", owner: SettingOwner::Client,
        label: "ownership.label.attributionPr", group: "ownership.group.filesAndGit",
        control: SettingControl::MultilineText,
    },
    InputSpec {
        app: AppKind::Codex,
        key: "tui.notifications",
        owner: SettingOwner::Client,
        label: "ownership.label.tuiNotifications",
        group: "ownership.group.terminalUi",
        control: SettingControl::Notifications,
    },
    InputSpec {
        app: AppKind::Codex,
        key: "service_tier",
        owner: SettingOwner::Provider,
        label: "ownership.label.serviceTier",
        group: "ownership.group.modelBehavior",
        control: SettingControl::Text,
    },
    InputSpec {
        app: AppKind::Codex,
        key: "model_auto_compact_token_limit",
        owner: SettingOwner::Provider,
        label: "ownership.label.autoCompactTokenLimit",
        group: "ownership.group.modelBehavior",
        control: SettingControl::Integer { min: 1, max: MAX_EXACT_CONFIG_INTEGER as u64 },
    },
    InputSpec {
        app: AppKind::Claude,
        key: "language",
        owner: SettingOwner::Client,
        label: "ownership.label.responseLanguage",
        group: "ownership.group.interfaceAndInteraction",
        control: SettingControl::Text,
    },
    InputSpec {
        app: AppKind::Claude,
        key: "autoMemoryDirectory",
        owner: SettingOwner::Client,
        label: "ownership.label.autoMemoryDirectory",
        group: "ownership.group.filesAndGit",
        control: SettingControl::Directory,
    },
    InputSpec {
        app: AppKind::Claude,
        key: "autoCompactWindow",
        owner: SettingOwner::Provider,
        label: "ownership.label.autoCompactWindow",
        group: "ownership.group.modelBehavior",
        control: SettingControl::Integer { min: COMPACT_WINDOW_MIN, max: COMPACT_WINDOW_MAX },
    },
    InputSpec {
        app: AppKind::Claude,
        key: "env.API_TIMEOUT_MS",
        owner: SettingOwner::Provider,
        label: "ownership.label.apiTimeoutMs",
        group: "ownership.group.modelBehavior",
        control: SettingControl::StringInteger { min: 1, max: 2_147_483_647 },
    },
    InputSpec {
        app: AppKind::Claude,
        key: "env.CLAUDE_CODE_MAX_OUTPUT_TOKENS",
        owner: SettingOwner::Provider,
        label: "ownership.label.maxOutputTokens",
        group: "ownership.group.modelBehavior",
        control: SettingControl::StringInteger { min: 1, max: MAX_EXACT_CONFIG_INTEGER as u64 },
    },
];
