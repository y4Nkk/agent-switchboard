use crate::contracts::AppKind;

/// The one ownership decision for a client configuration key. Unknown keys
/// are host-owned by default and therefore never enter a provider file or a
/// client-settings projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingOwner {
    Provider,
    Client,
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
    Integer,
    StringArray,
    BoolOrStringArray,
    StringOrObject,
}

/// What a provider projection does when the current provider has no value for
/// one of its own keys. This is deliberately explicit so a previous provider
/// cannot leak a routing or model field into the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAbsentAction {
    Remove,
}

/// Rendering metadata for an editable provider parameter or client setting.
/// Routing fields and host-owned structures have no scalar editor control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingControl {
    None,
    Toggle,
    Text,
    Directory,
    Integer { min: u64, max: u64 },
    StringInteger { min: u64, max: u64 },
    Notifications,
    StringList,
    MultilineText,
    ApprovalPolicy,
    Choice {
        presentation: ChoiceControl,
    },
}

/// One strongly typed entry in the ownership directory. `setting_spec` and
/// `setting_specs` are the only APIs consumers should use for ownership,
/// value constraints, UI metadata, and provider cleanup decisions.
#[derive(Debug, Clone)]
pub struct SettingSpec {
    pub app: AppKind,
    pub key: &'static str,
    pub owner: SettingOwner,
    pub value_type: SettingValueType,
    pub allowed_values: &'static [ChoiceOption],
    pub control: SettingControl,
    pub label: Option<&'static str>,
    pub group: Option<&'static str>,
    pub provider_absent_action: Option<ProviderAbsentAction>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ProviderSettingSpec {
    pub(super) app: AppKind,
    pub(super) key: &'static str,
    pub(super) value_type: SettingValueType,
}

/// One boolean general setting from the client's official configuration
/// reference.
#[derive(Debug, Clone, Copy)]
pub struct ToggleSpec {
    pub key: &'static str,
    pub owner: SettingOwner,
    pub label: &'static str,
    pub group: &'static str,
}

/// One selectable value of a multi-detent general setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceOption {
    pub value: &'static str,
    pub label: &'static str,
}

/// How the settings page renders a choice, including native custom values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceControl {
    Slider,
    Segment,
    Editable,
}

/// One multi-value general setting from the client's official configuration
/// reference.
#[derive(Debug, Clone, Copy)]
pub struct ChoiceSpec {
    pub key: &'static str,
    pub owner: SettingOwner,
    pub label: &'static str,
    pub group: &'static str,
    pub control: ChoiceControl,
    pub options: &'static [ChoiceOption],
}
