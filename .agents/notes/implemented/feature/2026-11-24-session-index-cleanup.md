# Agent Note: 会话与用量索引的显式清理

Status: implemented

## Problem

存储设置展示应用数据总量，已有维护入口只覆盖浏览器缓存和配置历史。实测 1.07 GiB 应用数据里，会话与用量索引约占 653 MiB，其中约 522 MiB 来自用量明细、去重调用及其索引。这个可重建的衍生数据库占用最大，用户无法从界面释放其空间。

## Decision

设置显示 `read-index.sqlite3` 及其日志文件的容量。确认清理后，读取索引所有者检查路径类型和数据库模式，持有索引打开门禁直到操作结束。退出 WAL 模式需要 SQLite 独占锁，现有连接还在使用索引时会拒绝清理。SQLite 关闭连接并回收日志后，清理目标限定为索引主文件。界面显示回收量并刷新容量。[SQLite WAL 锁](https://www.sqlite.org/walformat.html#how_the_various_locks_are_used)

会话页面与消耗统计继续从原始来源按需建立索引。Claude 会话保留和用户会话删除依照[会话保留](../../implemented/bug-fix/2026-10-22-claude-account-session-retention.md)及[会话日期区间操作](../../implemented/feature/2026-11-10-session-date-range-actions.md)维护。清理入口沿用[显式回收](../../implemented/architecture/2026-10-28-storage-reclamation-expansion.md)的设置归属，重建沿用[会话读取索引](../../implemented/architecture/2026-10-22-incremental-session-read-index.md)与[增量用量账本](../../implemented/architecture/2026-11-04-incremental-usage-ledger.md)。

## Alternatives considered

- 清空表后执行 `VACUUM` 能在同一文件里维持已知模式。清空和压缩分两个步骤，磁盘空间紧张时仍要为大库准备日志和压缩空间，部分成功需要额外处理。
- 启动时按容量重置可重建索引，能限制闲置库占用。较大的会话库每次重建都有扫描成本，自动清理可能使常用统计反复重建。用户确认可以把重建推迟到需要的时候。

## Consequences

清理会立即释放派生数据占用。重建需要重新扫描可用来源，首次打开可能较慢，索引会随使用再次增长。

隔离夹具覆盖活跃连接拒绝、未知模式保留、清理后的容量与重复清理、按需创建和同目录会话材料。按项目验证约定，本次执行笔记结构和源码差异检查，桌面运行与 Rust 夹具留待明确要求时执行。
