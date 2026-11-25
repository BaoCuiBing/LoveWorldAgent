# 技能（Skills）使用指南

技能是带附件的操作手册：`SKILL.md` 写说明，可附带脚本与参考资料。智能体平时只看到技能名称与用途，需要时再读取正文，按需加载、节省上下文。

## 技能是什么

- 技能 = 一个文件夹 + 一份 `SKILL.md`，告诉智能体"遇到某类任务时按什么步骤做"。
- 与规则不同：规则是约束（必须遵守），技能是方法（按需查阅）。
- 技能只读：对话中智能体只能读取技能，新增/修改需在技能管理页或 API 完成。

## 目录结构

```
src-tauri/static/skills/
└── weekly-report/
    ├── SKILL.md          # 技能定义（必需）
    ├── scripts/          # 可选：辅助脚本
    │   └── collect.py
    └── reference/        # 可选：参考资料
        └── template.md
```

- 文件夹名即技能名：英文字母开头，允许数字、点、横线、下划线。
- 没有 `SKILL.md` 的文件夹会被忽略。

## SKILL.md 结构

```markdown
---
name: weekly-report
description: 汇总本周会话与规则变更，生成周报
enabled: true
---

# 周报生成

## 步骤
1. 调用 file_read 读取 src-tauri/static/logs 下的本周日志。
2. 按"完成 / 风险 / 下周计划"三段输出。
3. 输出前先给出三行摘要。
```

### Frontmatter 字段

| 字段 | 说明 | 默认 |
| --- | --- | --- |
| `name` | 技能名，需与文件夹名一致 | 文件夹名 |
| `description` | 一句话用途，注入系统提示的核心信息 | 空 |
| `enabled` | 是否启用；`false/0/no/off` 视为停用 | `true` |

## 启用注入与按需读取

1. 启用技能的 `name + description` 以"## 可用技能"小节注入系统提示（停用技能不出现）。
2. 智能体判断某技能与任务相关时，调用 `skill_read` 工具读取正文。
3. 正文中的脚本与附件路径按相对路径描述，智能体可用 `file_read` 读取。

## 编写示例

最小可用技能：

```
skills/
└── code-review/
    └── SKILL.md
```

```markdown
---
name: code-review
description: 按团队清单审查 Python 代码
enabled: true
---

# 代码审查

1. 先读文件全文，再按清单逐项检查。
2. 清单：命名、异常处理、日志、类型标注。
3. 输出按"问题 / 建议 / 严重级别"三列表格。
```

## FAQ

**Q：技能里能放可执行脚本吗？**
A：可以，`scripts/` 目录放脚本，但当前版本智能体只会读取内容，不会自动执行。

**Q：修改技能后要重启吗？**
A：不用，每次对话都会重建智能体，保存后下次提问即生效。

**Q：为什么智能体说"看不到某个技能"？**
A：检查 frontmatter 的 `enabled` 是否为 true、`SKILL.md` 是否存在、文件夹名是否合法。

**Q：技能正文多长合适？**
A：建议单篇不超过 300 行；更长的资料拆到 `reference/` 按需读取。
