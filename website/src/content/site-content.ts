/** 官网可见文案与链接的唯一来源；界面截图来自隔离沙箱中的实际应用。 */
export type Locale = "zh-CN" | "en";

export interface SiteActionLink {
  label: string;
  href: string;
  variant: "primary" | "secondary";
  external: boolean;
  icon?: "github" | "star";
}

/** Application screenshots from isolated local frontends; see docs/screenshots. */
export interface SiteScreenshot {
  src: string;
  alt: string;
  badge: string;
  caption: string;
  /** Intrinsic pixel size of the PNG; keeps layout stable before load. */
  width: number;
  height: number;
}

/** One showcase section: copy on one side, a sandbox screenshot on the other. */
export interface SiteShowcase {
  title: string;
  description: string;
  bullets: string[];
  shot: SiteScreenshot;
}

/** One capability panel (text-only; never a software-UI replica). */
export interface SiteCapability {
  title: string;
  description: string;
  bullets: string[];
  /** Optional real sandbox screenshot shown beneath the copy. */
  shot?: SiteScreenshot;
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
    actions: SiteActionLink[];
    shot: SiteScreenshot;
  };
  preview: SiteShowcase;
  configuration: SiteShowcase;
  capabilities: {
    title: string;
    description: string;
    gateway: SiteCapability;
    subagent: SiteCapability;
  };
  writeLifecycle: {
    title: string;
    steps: string[];
    recovery: string;
  };
  final: { title: string; actions: SiteActionLink[] };
  footer: { note: string };
}

const repoUrl = "https://github.com/y4Nkk/agent-switchboard";
const releasesUrl = "https://github.com/y4Nkk/agent-switchboard/releases/latest";

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
      { label: "切换预览", href: "#preview", external: false },
      { label: "客户端配置", href: "#configuration", external: false },
      { label: "网关与子代理", href: "#capabilities", external: false },
      { label: "安全写入", href: "#safety", external: false },
    ],
    hero: {
      title: "拼好配置，再写入真实文件。",
      fact: "面向 Codex 与 Claude Code 的本地配置控制台。",
      actions: [
        { label: "GitHub", href: repoUrl, variant: "secondary", external: true, icon: "github" },
        { label: "下载版本", href: releasesUrl, variant: "primary", external: true },
      ],
      shot: {
        src: "/media/screenshots/providers.png",
        alt: "Agent Switchboard 的 Codex 与 Claude 供应商工作区，显示 Atlas、Lyra 演示连接和模拟余额",
        badge: "隔离沙箱 · 演示数据",
        caption: "当前连接、演示档案和模拟余额。",
        width: 1440,
        height: 1080,
      },
    },
    preview: {
      title: "写入前，先看变化。",
      description:
        "类型化差异预览列出每一个将写入的键，敏感值自动脱敏；只有点击确认，切换执行器才会触碰真实文件。",
      bullets: [
        "逐键差异：旧值 → 新值，未受管字段保持原样",
        "密钥与令牌自动脱敏，预览不泄露凭据",
        "确认后才写入，全程备份、校验、可恢复",
      ],
      shot: {
        src: "/media/screenshots/switch-preview.png",
        alt: "从 Atlas 到 Nova 的模拟切换预览，显示模型、推理强度和端点差异，以及候选文件和备份位置",
        badge: "隔离沙箱 · 演示数据",
        caption: "在同一处核对模拟差异、候选文件和备份位置。",
        width: 1440,
        height: 1400,
      },
    },
    configuration: {
      title: "客户端配置，集中受控。",
      description:
        "按组编辑两端的通用配置、子 agent 运行设置和全局指令。设置变更可在应用前集中预览。",
      bullets: [
        "以用户文件为基准，只提交明确修改的字段",
        "检查本机配置来源及 Codex 默认权限档案",
        "恢复原生默认值时可逐项查看并选择额外清理字段",
      ],
      shot: {
        src: "/media/screenshots/client-configuration.png",
        alt: "分组客户端配置的模拟界面，显示模型行为、子 agent 开关、并发数和默认权限档案",
        badge: "隔离沙箱 · 演示数据",
        caption: "模型行为、子 agent 和权限档案按组排列。",
        width: 1440,
        height: 1080,
      },
    },
    capabilities: {
      title: "网关与子代理，边界清晰。",
      description:
        "协议转换只发生在本机回环；跨档案的子代理路由也经同一网关，凭据与计量不越过这条边界。",
      gateway: {
        title: "本机网关，只在需要时启动。",
        description:
          "第三方上游协议与 Codex / Claude Code 协议不一致时，应用只在 127.0.0.1 启动本地转换网关；跨供应商的子代理请求也必须经它路由。",
        bullets: [
          "仅在协议不一致时启动，平时不驻留",
          "只监听 127.0.0.1 本机回环，不提供公开代理",
          "无遥测、无云中转，转换全部在本机完成",
        ],
      },
      subagent: {
        title: "图形化模型路由。",
        description:
          "点击「客户端 → 用途 → 供应商与模型」的目标节点，配置 Codex 主模型、默认子 agent 或 Claude 模型档位，预览后确认应用。",
        bullets: [
          "写入 config.toml 的永远是路由引用 asb:<档案>/<模型>",
          "跨供应商子 agent 请求使用本机网关与目标档案的连接",
          "目标档案凭据按请求替换，计量按实际目标归因",
          "不参与跨供应商故障转移：目标档案失败即失败",
        ],
        shot: {
          src: "/media/screenshots/model-routing.png",
          alt: "供应商编辑器的模型去向，默认子 agent 指向 Orion 演示档案的 orion-mini",
          badge: "隔离沙箱 · 演示数据",
          caption: "在供应商编辑器内选择主模型和默认子 agent 的去向。",
          width: 1440,
          height: 1080,
        },
      },
    },
    writeLifecycle: {
      title: "确认后，按事务写入。",
      steps: ["创建备份", "临时写入", "临时回读", "语法校验", "最终冲突检查", "原子替换", "写后验证"],
      recovery: "写后验证失败时，恢复刚创建的备份",
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
      { label: "Preview", href: "#preview", external: false },
      { label: "Client settings", href: "#configuration", external: false },
      { label: "Gateway & sub-agents", href: "#capabilities", external: false },
      { label: "Safe writes", href: "#safety", external: false },
    ],
    hero: {
      title: "Compose settings. Write real files.",
      fact: "A local configuration console for Codex and Claude Code.",
      actions: [
        { label: "GitHub", href: repoUrl, variant: "secondary", external: true, icon: "github" },
        { label: "Download", href: releasesUrl, variant: "primary", external: true },
      ],
      shot: {
        src: "/media/screenshots/providers.png",
        alt: "Agent Switchboard providers workspace showing Atlas and Lyra demo connections for Codex and Claude with simulated balances",
        badge: "Isolated sandbox · demo data",
        caption: "Active connections, demo profiles, and simulated balances.",
        width: 1440,
        height: 1080,
      },
    },
    preview: {
      title: "Inspect changes before writing.",
      description:
        "A typed diff preview lists every key about to be written, with sensitive values redacted; the switch executor touches real files only after you confirm.",
      bullets: [
        "Key-by-key diff — old → new, unmanaged fields untouched",
        "Keys and tokens redacted, so previews never leak credentials",
        "Writes only after confirmation — backed up, validated, recoverable",
      ],
      shot: {
        src: "/media/screenshots/switch-preview.png",
        alt: "Mock switch from Atlas to Nova showing model, reasoning, and endpoint changes, the candidate file, and the backup location",
        badge: "Isolated sandbox · demo data",
        caption: "Review the simulated diff, candidate file, and backup location together.",
        width: 1440,
        height: 1400,
      },
    },
    configuration: {
      title: "Client settings, under control.",
      description:
        "Manage shared settings, subagent options, and global instructions in grouped forms. Review changes together before applying them.",
      bullets: [
        "Start from the user file and apply only the fields you changed",
        "Inspect configuration sources and the default Codex permission profile",
        "Preview and select additional fields when restoring native defaults",
      ],
      shot: {
        src: "/media/screenshots/client-configuration.png",
        alt: "Grouped client configuration with simulated model behavior, subagent concurrency, and a default permission profile",
        badge: "Isolated sandbox · demo data",
        caption: "Grouped model behavior, subagents, and permissions.",
        width: 1440,
        height: 1080,
      },
    },
    capabilities: {
      title: "Gateway and sub-agents, clear boundaries.",
      description:
        "Protocol conversion happens on the loopback only; cross-profile sub-agent routing goes through the same gateway, with credentials and metering kept inside that boundary.",
      gateway: {
        title: "A local gateway, started only when needed.",
        description:
          "When a third-party upstream speaks a different protocol than Codex or Claude Code, the app starts a local conversion gateway on 127.0.0.1 only; cross-provider sub-agent requests must route through it too.",
        bullets: [
          "Started only on protocol mismatch — nothing resident otherwise",
          "Loopback-only listener on 127.0.0.1, never a public proxy",
          "No telemetry, no cloud relay — conversion stays on this machine",
        ],
      },
      subagent: {
        title: "Graphical model routing.",
        description:
          "Select a destination in client → purpose → provider and model to configure the Codex main model, default sub-agent or Claude tier, then preview and confirm.",
        bullets: [
          "config.toml always receives the route reference asb:<profile>/<model>",
          "Cross-provider sub-agent requests use the local gateway and target profile's connection",
          "Target-profile credentials swapped per request; usage metered to the actual target",
          "No cross-provider failover: target-profile failure is failure",
        ],
        shot: {
          src: "/media/screenshots/model-routing.png",
          alt: "Provider editor model destinations with the default subagent set to orion-mini in the Orion demo profile",
          badge: "Isolated sandbox · demo data",
          caption: "Choose the main model and default subagent destination in the provider editor.",
          width: 1440,
          height: 1080,
        },
      },
    },
    writeLifecycle: {
      title: "After confirmation, the transaction writes.",
      steps: [
        "Create backup",
        "Write temporary file",
        "Verify temporary file",
        "Validate syntax",
        "Final conflict check",
        "Atomic replace",
        "Verify write",
      ],
      recovery: "If post-write verification fails, restore the just-created backup",
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

export function actionProps(action: { href: string; external: boolean }) {
  return action.external
    ? { href: action.href, target: "_blank", rel: "noreferrer" }
    : { href: action.href };
}
