# Agent Note: Claude 网关事务账本

Status: implemented

## Problem

Claude 网关记录结算时重新解析、校验和原子改写整份 JSON。已配置预算的请求也读取明细才能检查已累计费用。

## Decision

- `claude-request-ledger.sqlite3` 是网关计费和明细的持久账本。每次结算在一个 SQLite 事务内追加脱敏明细、累计 UTC 日费用、裁剪到 10000 条明细和 400 个计费日。预算查询按供应商和 UTC 月范围读取索引，按现有规则得出日与月两个金额。
- 数据库通过应用标识和版本确认所有权。空库在同一事务内创建结构、导入合法旧账本并提交标识。第 2 版导入现有累计费用，保留已淘汰明细的贡献；第 1 版从现有明细重建费用。旧 JSON 保留为升级前证据，已初始化的库直接使用 SQLite。

## Related decisions

[请求性能统一使用客户端原生 OpenTelemetry](2026-11-10-native-opentelemetry-request-performance.md)拥有性能报表和原生观测。本篇负责 Claude 网关单独的计费事实。

## Alternatives considered

- 只把费用摘要放在内存里，能避免预算路径反复解析。记录写入仍需改写整份历史，还要维护持久提交与摘要发布的一致性。
- 用 JSONL 追加，改动较小。保留期修剪、按日累计与进程中断后恢复需要额外索引和跨文件提交协议。

## Consequences

首次触及账本时执行一次性初始化。SQLite 事务回滚保留完整旧账本与初始化重试能力。预算与费用写入共享连接，写入错误继续由网关记录。回归用例涵盖累计费用迁移、初始化失败重试和费用溢出回滚，当前只做静态检查。

## Primary references

- [SQLite 原子提交](https://sqlite.org/atomiccommit.html)
