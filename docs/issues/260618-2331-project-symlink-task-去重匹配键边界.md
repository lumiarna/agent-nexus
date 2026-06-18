# Project Symlink 去重匹配键的语义边界

## 问题

`ProjectSymlinkInventory::list_project_symlinks` 当前用 **target 路径** 作为去重匹配键：只要某个 `Symlink`/`Junction` + `Local` target 的 task 存在于 `tasks` 表，inventory 就隐藏该 target 路径上扫描到的任何 symlink（见 `crates/nexus-core/src/services/project_symlinks.rs` 的 `managed_link_targets` + `retain`）。

这符合「已出现在 task 里的软链关系不用冗余显示」的原始表述，但隐含一个语义边界：**去重判定的是 target 路径声明权，不是同一 symlink 关系**。

## 副作用场景

1. 用户用 task 创建了 `source-A → target-X` 的 symlink。
2. 用户手动删除 `target-X` 的 symlink（`link_state` 变 `missing`，task 行仍在）。
3. 用户手动在同 `target-X` 路径建一个指向 **不同 source** 的新 symlink `source-B → target-X`。
4. `list_project_symlinks` 会**隐藏**这个新 link——因为 task 还「占着」`target-X`，尽管新 link 的 source 与 task 声明的 source 已不一致。

结果：用户在 Project Symlinks 区块看不到自己刚建的新 link，且无任何提示。

## 替代方案

双键匹配 `(canonicalize(source), canonicalize(target))`：只有 source 和 target 都匹配 task 声明时才隐藏。

| 方案 | 优 | 劣 |
|---|---|---|
| 当前 target-only | 实现简单；倾向隐藏，避免冗余 | 上述副作用场景下用户看不到自己建的新 link |
| 双键 (source, target) | 精确匹配 task 声明的关系；副作用场景下新 link 会出现 | task 的 source 路径与扫描到的 source 路径字符串形式不同时（如 `~/foo` vs `/Users/me/foo`、相对 vs 绝对）会漏判，产生 false-positive 冗余显示 |

两个方案都"保守"，方向相反：target-only 倾向隐藏，双键倾向显示。

## 决策

暂不改动。当前 target-only 方案符合「task 管理的 target 不冗余显示」的原始意图，副作用场景属边角情况（用户手动在同路径换 source 的概率低）。记录备查，待真实使用中暴露该场景再 revisit。

## 备注

- 若未来改为双键匹配，`managed_link_targets` 需同时返回 `(source, target)` 对，`retain` 判定改为 `managed_pairs.contains(&(normalise(&link.source_path), normalise(&link.target_path)))`。
- 若引入「Re-create placement」动作（从 MISSING task 重建 symlink），该动作会覆盖 target 路径上任何现存内容——此时 target-only 语义反而更安全（task 声明权明确）。这是倾向维持 target-only 的另一理由。
