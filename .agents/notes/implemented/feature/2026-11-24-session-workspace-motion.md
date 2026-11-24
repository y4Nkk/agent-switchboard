# Agent Note: 会话工作区的过渡动效

Status: implemented

## Problem

会话页签、结果列表和阅读区在切换时直接替换，批量操作和筛选弹层同样立即出现。页头、筛选行和列表需要清晰的视觉层次，异步搜索进度与批量勾选也需要独立反馈。

## Decision

会话与已归档通过当前页签决定同一正文的入场关键帧，片段收藏在显示时使用同一动效时长。搜索、收藏、批量列表和消息正文在当前内容出现时淡入，日期处理的预览和结果使用同一方式。选中、高亮与悬停通过背景过渡反馈。自定义日期沿用共享 Collapsible，字段间留白随内容展开，收起过程中字段不可交互。

会话工作区通过 OverlayMotionContext 给共享 AppDialog、ActionMenu、Select 和 DatePicker 标记动效范围。过渡使用全局时长与缓动。菜单退场由 React Aria 的弹层状态驱动，会话对话框及字段下拉沿用调用方的关闭生命周期。对话框使用透明度入场，以保留内部字段弹层的视口定位。

## Related decisions

- [会话工作区的操作与检索布局](../../implemented/feature/2026-11-24-session-workspace-controls.md)拥有页头与列表的布局。
- [会话弹窗控件与状态的归属](../../implemented/architecture/2026-11-24-session-dialog-control-boundaries.md)拥有弹层容器、关闭顺序、日期草稿与批量选择。
- [工作区页签共用可取消的滑动动画](../../implemented/architecture/2026-10-16-client-route-transition-owner.md)拥有独立正文面板的双端动画和动效时长。
- [全局折叠原语](../../implemented/architecture/2026-10-14-unified-collapsible-primitive.md)拥有展开和收起生命周期。

## Alternatives considered

- 给全部页签保留进出双端，可复用其它工作区的连续反向滑动。会话和已归档共用搜索与阅读状态，双端同时播放需要保存旧结果或增加活跃面板，当前正文入场保持单一状态归属。
- 统一延后关闭全部弹窗，可播放完整退场。关闭动作可能处理批量状态、焦点或异步范围计划清理，视图淡入和菜单自身退场可以沿用现有生命周期。

## Consequences

- 页头、检索行和分页保持在各自的布局中。查询、阅读和批量状态由原组件持有，已展示内容上的进度和选中变更只更新对应状态。
- 页签展示当前正文的入场，弹窗和字段选择器按已有时机关闭，菜单可完整播放退场。减少动态效果时，全局动效时长归零。
- 验证范围包括笔记结构、差异和相关源码语法。桌面 WebView 中的键盘焦点、快速切换、嵌套确认及动效偏好需要运行验收。
