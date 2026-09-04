import type {
  ConfigurationAssemblyClientId,
  ConfigurationAssemblyControlValue,
  ConfigurationAssemblyFieldKey,
} from "../generated/configuration-assembly";

/** 官网可见文案与链接的唯一来源；配置证据由桌面端适配器生成。 */
export type Locale = "zh-CN" | "en";
export type RelayTone = "codex" | "claude";

export interface SiteActionLink {
  label: string;
  href: string;
  variant: "primary" | "secondary";
  external: boolean;
  icon?: "github" | "star";
}

export interface RouteCardData {
  client: string;
  tone: RelayTone;
  provider: string;
  model: string;
  endpoint: string;
  access: string;
}

export interface StatusCardData {
  client: string;
  tone: RelayTone;
  status: string;
  rows: Array<[string, string]>;
}

interface AssemblyClientCopy {
  commonTitle: string;
  providerTitle: string;
  fileNote: string;
}

export interface SiteContent {
  brand: string;
  repoUrl: string;
  releasesUrl: string;
  document: { title: string; description: string };
  header: {
    navigationLabel: string;
    localeMenuLabel: string;
    themeToDarkLabel: string;
    themeToLightLabel: string;
    githubLabel: string;
    locales: Array<{ id: Locale; short: string; label: string }>;
  };
  nav: Array<{ label: string; href: string; external: boolean }>;
  hero: {
    title: string;
    fact: string;
    routeCardLabel: (client: string) => string;
    routeFieldLabels: { model: string; endpoint: string; access: string };
    actions: SiteActionLink[];
    appShell: {
      nav: string[];
      enabledPanel: string;
      statusPanel: string;
      statusAction: string;
    };
    cards: RouteCardData[];
    status: StatusCardData[];
  };
  assembly: {
    title: string;
    description: string;
    clientSelectorLabel: string;
    combineLabel: string;
    fieldLabels: Record<ConfigurationAssemblyFieldKey, string>;
    controlLabels: Record<ConfigurationAssemblyControlValue, string>;
    clients: Record<ConfigurationAssemblyClientId, AssemblyClientCopy>;
  };
  preview: {
    title: string;
    description: string;
    changes: Array<{ key: string; from: string; to: string }>;
    file: string;
    fileNote: string;
    codeLines: string[];
    cancelLabel: string;
    confirmLabel: string;
  };
  final: { title: string; actions: SiteActionLink[] };
  footer: { note: string };
}

const repoUrl = "https://github.com/y4Nkk/agent-switchboard";
const releasesUrl = "https://github.com/y4Nkk/agent-switchboard/releases/latest";

const sharedRouteCards: RouteCardData[] = [
  {
    client: "Codex",
    tone: "codex",
    provider: "Amazon Bedrock",
    model: "GPT-6 Astra",
    endpoint: "bedrock-runtime.us-east-1.amazonaws.com",
    access: "自定义",
  },
  {
    client: "Claude",
    tone: "claude",
    provider: "Amazon Bedrock",
    model: "Claude Opus 5.1",
    endpoint: "bedrock-runtime.us-east-1.amazonaws.com",
    access: "自定义",
  },
];

export const siteContentByLocale: Record<Locale, SiteContent> = {
  "zh-CN": {
    brand: "Agent Switchboard",
    repoUrl,
    releasesUrl,
    document: {
      title: "Agent Switchboard — 拼好配置，再写入真实文件",
      description: "面向 Codex 与 Claude Code 的本地配置控制台。",
    },
    header: {
      navigationLabel: "页面导航",
      localeMenuLabel: "选择语言",
      themeToDarkLabel: "切换到深色外观",
      themeToLightLabel: "切换到浅色外观",
      githubLabel: "在 GitHub 查看 Agent Switchboard",
      locales: [
        { id: "zh-CN", short: "CN", label: "中文" },
        { id: "en", short: "US", label: "English" },
      ],
    },
    nav: [
      { label: "配置如何生成", href: "#assembly", external: false },
      { label: "预览", href: "#preview", external: false },
    ],
    hero: {
      title: "拼好配置，再写入真实文件。",
      fact: "面向 Codex 与 Claude Code 的本地配置控制台。",
      routeCardLabel: (client) => `${client} 当前配置`,
      routeFieldLabels: { model: "模型", endpoint: "服务地址", access: "接入方式" },
      actions: [
        { label: "GitHub", href: repoUrl, variant: "secondary", external: true, icon: "github" },
        { label: "下载版本", href: releasesUrl, variant: "primary", external: true },
      ],
      appShell: {
        nav: ["概览", "供应商", "通用设置", "用量", "会话", "日志", "备份", "发现", "设置"],
        enabledPanel: "当前启用配置",
        statusPanel: "配置状态",
        statusAction: "刷新状态",
      },
      cards: sharedRouteCards,
      status: [
        {
          client: "Codex",
          tone: "codex",
          status: "配置正常",
          rows: [
            ["配置文件", "C:\\Users\\demo\\.codex\\config.toml"],
            ["当前服务", "Amazon Bedrock · GPT-6 Astra"],
            ["匹配状态", "与档案「Amazon Bedrock」一致"],
            ["上次切换", "2026年09月04日 16:20 · Amazon Bedrock"],
            ["写入锁", "写入锁空闲"],
          ],
        },
        {
          client: "Claude",
          tone: "claude",
          status: "配置正常",
          rows: [
            ["配置文件", "C:\\Users\\demo\\.claude\\settings.json"],
            ["当前服务", "Amazon Bedrock · Claude Opus 5.1"],
            ["匹配状态", "与档案「Amazon Bedrock」一致"],
            ["上次切换", "2026年09月04日 16:20 · Amazon Bedrock"],
            ["写入锁", "写入锁空闲"],
          ],
        },
      ],
    },
    assembly: {
      title: "配置由组件组成，文件由适配器渲染。",
      description: "通用配置与供应商设置各自只投影到受管字段。",
      clientSelectorLabel: "选择客户端示例",
      combineLabel: "合成实际配置文件",
      fieldLabels: {
        model_reasoning_effort: "推理强度",
        hide_agent_reasoning: "隐藏推理摘要",
        model: "模型",
        model_provider: "模型提供方",
        openai_base_url: "服务地址",
        effortLevel: "推理强度",
        autoCompactEnabled: "自动压缩上下文",
        "env.ANTHROPIC_BASE_URL": "服务地址",
        "env.ANTHROPIC_AUTH_TOKEN": "访问密钥",
      },
      controlLabels: {
        automatic: "自动",
        true: "开启",
        false: "关闭",
        minimal: "极低",
        low: "低",
        medium: "中",
        high: "高",
        xhigh: "极高",
      },
      clients: {
        codex: {
          commonTitle: "Codex 通用配置",
          providerTitle: "Amazon Bedrock 设置",
          fileNote: "由桌面端适配器生成；密钥已脱敏",
        },
        claude: {
          commonTitle: "Claude Code 通用配置",
          providerTitle: "Amazon Bedrock 设置",
          fileNote: "由桌面端适配器生成；密钥已脱敏",
        },
      },
    },
    preview: {
      title: "写入前，先看变化。",
      description: "真实差异预览，敏感值自动脱敏。",
      changes: [
        { key: "model", from: "gpt-5-codex", to: "GPT-6 Astra" },
        {
          key: "openai_base_url",
          from: "https://relay.example.com/v1",
          to: "https://bedrock-runtime.us-east-1.amazonaws.com/v1",
        },
        { key: "api key", from: "••••••••", to: "••••••••" },
      ],
      file: "C:\\Users\\demo\\.codex\\config.toml",
      fileNote: "17 行",
      codeLines: [
        'model = "GPT-6 Astra"',
        'model_provider = "openai"',
        'openai_base_url = "https://bedrock-runtime.us-east-1.amazonaws.com/v1"',
        "# —— 其余宿主自有键保持原样 ——",
      ],
      cancelLabel: "取消",
      confirmLabel: "确认切换",
    },
    final: {
      title: "从 GitHub 开始使用",
      actions: [
        { label: "查看源码", href: repoUrl, variant: "secondary", external: true, icon: "github" },
        { label: "点亮 Star", href: repoUrl, variant: "primary", external: true, icon: "star" },
      ],
    },
    footer: {
      note: "以 MIT License 开源。Codex 与 Claude Code 是其各自权利人的商标；Agent Switchboard 与 OpenAI、Anthropic 无隶属或认可关系。",
    },
  },
  en: {
    brand: "Agent Switchboard",
    repoUrl,
    releasesUrl,
    document: {
      title: "Agent Switchboard — Compose settings. Write real files.",
      description: "A local configuration console for Codex and Claude Code.",
    },
    header: {
      navigationLabel: "Page navigation",
      localeMenuLabel: "Choose language",
      themeToDarkLabel: "Use dark appearance",
      themeToLightLabel: "Use light appearance",
      githubLabel: "View Agent Switchboard on GitHub",
      locales: [
        { id: "zh-CN", short: "CN", label: "中文" },
        { id: "en", short: "US", label: "English" },
      ],
    },
    nav: [
      { label: "Configuration", href: "#assembly", external: false },
      { label: "Preview", href: "#preview", external: false },
    ],
    hero: {
      title: "Compose settings. Write real files.",
      fact: "A local configuration console for Codex and Claude Code.",
      routeCardLabel: (client) => `${client} current configuration`,
      routeFieldLabels: { model: "Model", endpoint: "Endpoint", access: "Route" },
      actions: [
        { label: "GitHub", href: repoUrl, variant: "secondary", external: true, icon: "github" },
        { label: "Download", href: releasesUrl, variant: "primary", external: true },
      ],
      appShell: {
        nav: ["Overview", "Providers", "Common", "Usage", "Sessions", "Logs", "Backups", "Discovery", "Settings"],
        enabledPanel: "Active configuration",
        statusPanel: "Configuration status",
        statusAction: "Refresh",
      },
      cards: sharedRouteCards.map((card) => ({ ...card, access: "Custom" })),
      status: [
        {
          client: "Codex",
          tone: "codex",
          status: "Healthy",
          rows: [
            ["Config file", "C:\\Users\\demo\\.codex\\config.toml"],
            ["Current service", "Amazon Bedrock · GPT-6 Astra"],
            ["Match", "Matches Amazon Bedrock"],
            ["Last switch", "Sep 04, 2026 16:20 · Amazon Bedrock"],
            ["Write lock", "Idle"],
          ],
        },
        {
          client: "Claude",
          tone: "claude",
          status: "Healthy",
          rows: [
            ["Config file", "C:\\Users\\demo\\.claude\\settings.json"],
            ["Current service", "Amazon Bedrock · Claude Opus 5.1"],
            ["Match", "Matches Amazon Bedrock"],
            ["Last switch", "Sep 04, 2026 16:20 · Amazon Bedrock"],
            ["Write lock", "Idle"],
          ],
        },
      ],
    },
    assembly: {
      title: "Components in. Native files out.",
      description: "Common and provider settings each project only their managed fields.",
      clientSelectorLabel: "Choose a client example",
      combineLabel: "Compose the native configuration file",
      fieldLabels: {
        model_reasoning_effort: "Reasoning effort",
        hide_agent_reasoning: "Hide reasoning",
        model: "Model",
        model_provider: "Model provider",
        openai_base_url: "Endpoint",
        effortLevel: "Reasoning effort",
        autoCompactEnabled: "Auto compact",
        "env.ANTHROPIC_BASE_URL": "Endpoint",
        "env.ANTHROPIC_AUTH_TOKEN": "Access token",
      },
      controlLabels: {
        automatic: "Automatic",
        true: "On",
        false: "Off",
        minimal: "Minimal",
        low: "Low",
        medium: "Medium",
        high: "High",
        xhigh: "Very high",
      },
      clients: {
        codex: {
          commonTitle: "Codex common settings",
          providerTitle: "Amazon Bedrock settings",
          fileNote: "Generated by the desktop adapter; secrets are redacted",
        },
        claude: {
          commonTitle: "Claude Code common settings",
          providerTitle: "Amazon Bedrock settings",
          fileNote: "Generated by the desktop adapter; secrets are redacted",
        },
      },
    },
    preview: {
      title: "Inspect changes before writing.",
      description: "A real diff preview keeps sensitive values redacted.",
      changes: [
        { key: "model", from: "gpt-5-codex", to: "GPT-6 Astra" },
        {
          key: "openai_base_url",
          from: "https://relay.example.com/v1",
          to: "https://bedrock-runtime.us-east-1.amazonaws.com/v1",
        },
        { key: "api key", from: "••••••••", to: "••••••••" },
      ],
      file: "C:\\Users\\demo\\.codex\\config.toml",
      fileNote: "17 lines",
      codeLines: [
        'model = "GPT-6 Astra"',
        'model_provider = "openai"',
        'openai_base_url = "https://bedrock-runtime.us-east-1.amazonaws.com/v1"',
        "# Host-owned keys remain unchanged",
      ],
      cancelLabel: "Cancel",
      confirmLabel: "Confirm switch",
    },
    final: {
      title: "Start from GitHub",
      actions: [
        { label: "View source", href: repoUrl, variant: "secondary", external: true, icon: "github" },
        { label: "Star on GitHub", href: repoUrl, variant: "primary", external: true, icon: "star" },
      ],
    },
    footer: {
      note: "Released under the MIT License. Codex and Claude Code are trademarks of their respective owners; Agent Switchboard is not affiliated with or endorsed by OpenAI or Anthropic.",
    },
  },
};

/** 默认中文内容供静态契约测试使用。运行时由 SitePreferences 选择语言。 */
export const siteContent = siteContentByLocale["zh-CN"];

export function actionProps(action: { href: string; external: boolean }) {
  return action.external
    ? { href: action.href, target: "_blank", rel: "noreferrer" }
    : { href: action.href };
}
