import type { MessageEntry } from "../types";

export const messages = {
  "codex.probe.backgroundNote": ["可切换页面或最小化窗口，检测会继续运行；结束后通过标题栏全局信息通知。退出应用会停止检测。", "You can switch pages or minimize the window while the probe runs. Results appear in the titlebar global information. Exiting the app stops the probe."],
  "codex.probe.notify.completed": ["降智雷达检测完成", "Degradation radar probe finished"],
  "codex.probe.notify.graded": ["{question}：{passed}/{judged} 次通过，共 {total} 次；请到降智雷达查看详情。", "{question}: {passed}/{judged} runs passed, {total} runs in total. Open Degradation radar for details."],
  "codex.probe.notify.visual": ["{question}：回答已保存，请到降智雷达观察结果。", "{question}: answers saved. Open Degradation radar to review the results."],
  "codex.probe.notify.visualIssues": ["{question}：执行中存在错误，请到降智雷达查看详情。", "{question}: execution errors occurred. Open Degradation radar for details."],
  "codex.probe.notify.cancelled": ["降智雷达检测已取消", "Degradation radar probe cancelled"],
  "codex.probe.notify.failed": ["降智雷达检测失败", "Degradation radar probe failed"],
  "codex.probe.notify.configChanged": ["配置变化，降智雷达已停止检测", "Configuration changed; Degradation radar stopped"],
  "codex.probe.notify.interrupted": ["降智雷达检测已中断", "Degradation radar probe interrupted"],
  "codex.probe.notify.unsaved": ["降智雷达已停止，结果尚未保存；请到雷达页面重试保存", "Degradation radar stopped with unsaved results; open the radar to retry saving"],
  "codex.probe.notify.saved": ["降智雷达检测结果已保存", "Degradation radar results saved"],
  "codex.probe.notify.readFailed": ["无法读取降智雷达状态，正在重试", "Cannot read Degradation radar status; retrying"],
  "codex.probe.notify.watchFailed": ["降智雷达通知订阅失败，正在重试", "Cannot subscribe to Degradation radar notifications; retrying"],
  "codex.probe.notify.startFailed": ["无法开始降智雷达检测", "Cannot start Degradation radar probe"],
  "codex.probe.notify.cancelFailed": ["无法取消降智雷达检测", "Cannot cancel Degradation radar probe"],
  "codex.probe.notify.saveFailed": ["无法保存降智雷达检测结果", "Cannot save Degradation radar results"],
} as const satisfies Record<string, MessageEntry>;
