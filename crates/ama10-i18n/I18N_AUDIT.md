# ZetaCode IDE i18n 审计清单

> 本文件列出了 IDE 界面中所有硬编码的英文用户界面字符串，按 crate 和文件组织。
> 每条目标记了行号和上下文说明，便于查找和提取。
> 总计约 **2,000+** 字符串，跨越 **25+ crate**。

---

## 目录

1. [workspace](#1-workspace)
2. [zed (app_menus)](#2-zed-app_menus)
3. [editor](#3-editor)
4. [project_panel](#4-project_panel)
5. [git_ui](#5-git_ui)
6. [collab_ui](#6-collab_ui)
7. [terminal_view](#7-terminal_view)
8. [settings_ui](#8-settings_ui)
9. [extensions_ui](#9-extensions_ui)
10. [command_palette / file_finder / search](#10-command_palette--file_finder--search)
11. [agent_ui](#11-agent_ui)
12. [diagnostics](#12-diagnostics)
13. [outline_panel](#13-outline_panel)
14. [tasks_ui](#14-tasks_ui)
15. [onboarding](#15-onboarding)
16. [title_bar](#16-title_bar)
17. [auto_update_ui](#17-auto_update_ui)
18. [recent_projects](#18-recent_projects)
19. [theme_selector](#19-theme_selector)
20. [notifications](#20-notifications)
21. [ama10-ui](#21-ama10-ui)
22. [杂项 crate](#22-杂项-crate)

---

## 1. workspace

### `crates/workspace/src/pane.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 2240 | `"This file has changed on disk since you started editing it. Do you want to overwrite it?"` | `CONFLICT_MESSAGE` 常量 |
| 2242 | `"This file has been deleted on disk since you started editing it. Do you want to recreate it?"` | `DELETED_MESSAGE` 常量 |
| 1995-2001 | `"Do you want to save changes to the following files?"` | 对话框标题 |
| 1995-2001 | `"Save all"` / `"Discard all"` / `"Cancel"` | 对话框按钮 |
| 2037-2043 | `"Unable to save file: {err}"` | 警告标题 |
| 2037-2043 | `"Close Without Saving"` / `"Cancel"` | 对话框按钮 |
| 2307-2313 | `"Save"` / `"Close"` / `"Cancel"` | 对话框按钮 |
| 2342-2348 | `"Overwrite"` / `"Discard"` / `"Cancel"` | 对话框按钮 |
| 2385-2391 | `"Save"` / `"Don't Save"` / `"Cancel"` | 对话框按钮 |
| 2873-2878 | `"Unlock File"` / `"This will make this file editable"` | 标签锁工具提示 |
| 2879-2880 | `"Locked File"` / `"This file is read-only"` | 标签锁工具提示 |
| 2988-2998 | `"Unpin Tab"` | 标签尾部工具提示 |
| 3004 | `"Close Tab"` | 标签尾部工具提示 |
| 3059 | `"Read-Only File"` | 只读文件工具提示 |
| 3126 | `"Close"` | 标签上下文菜单 |
| 3134 | `"Close Others"` | 标签上下文菜单 |
| 3150 | `"Close Multibuffers"` | 标签上下文菜单 |
| 3167 | `"Close Left"` | 标签上下文菜单 |
| 3181 | `"Close Right"` | 标签上下文菜单 |
| 3196 | `"Close Clean"` | 标签上下文菜单 |
| 3209 | `"Close All"` | 标签上下文菜单 |
| 3221 | `"Unpin Tab"` | 标签上下文菜单 |
| 3229 | `"Pin Tab"` | 标签上下文菜单 |
| 3240-3243 | `"Make File Read-Only"` / `"Make File Editable"` | 标签上下文菜单 |
| 3302 | `"Copy Path"` | 标签上下文菜单 |
| 3313 | `"Copy Relative Path"` | 标签上下文菜单 |
| 3349 | `"Reveal In Project Panel"` | 标签上下文菜单 |
| 3364 | `"Open in Terminal"` | 标签上下文菜单 |
| 3416-3423 | `"Go Back"` | 后退按钮工具提示 |
| 3439-3446 | `"Go Forward"` | 前进按钮工具提示 |
| 4060-4063 | `"Cannot drop files on a remote project"` | 错误通知 |
| 4213-4215 | `"New..."` | "+" 按钮工具提示 |
| 4221-4231 | `"New File"`, `"Open File"`, `"Search Project"`, `"Search Symbols"`, `"New Terminal"`, `"New Center Terminal"` | "+" 弹窗菜单 |
| 4237-4241 | `"Split Pane"` | 分屏按钮工具提示 |
| 4249-4257 | `"Split Right"`, `"Split Left"`, `"Split Up"`, `"Split Down"` | 分屏弹窗菜单 |
| 4272-4278 | `"Zoom Out"` / `"Zoom In"` | 缩放按钮工具提示 |
| 4900 | `"This buffer"` | 脏缓冲区回退路径名 |

### `crates/workspace/src/workspace.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 3305-3311 | `"Do you want to leave the current call?"` | 警告对话框标题 |
| 3305-3311 | `"Close window and hang up"` / `"Cancel"` | 对话框按钮 |
| 3553-3559 | `"Do you want to save all changes in the following files?"` | 警告对话框标题 |
| 3553-3559 | `"Save all"` / `"Discard all"` / `"Cancel"` | 对话框按钮 |
| 3858-3861 | `"You cannot add folders to someone else's project"` | 错误通知 |
| 8518-8519 | `"Failed to load the database file."` / `"File an Issue"` | 通知 + 按钮 |
| 9457-9463 | `"Do you want to switch channels?"` / `"Leaving this call will unshare your current project."` | 警告对话框 |
| 9457-9463 | `"Yes, Join Channel"` / `"Cancel"` | 对话框按钮 |
| 9647-9653 | `"Failed to join channel"` / `"OK"` | 错误对话框 |
| 10624-10630 | `"Are you sure you want to restart?"` / `"Restart"` / `"Cancel"` | 信息对话框 |

### `crates/workspace/src/welcome.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 451-453 | `"Welcome back to ZetaCode"` / `"Welcome to ZetaCode"` | 标题 |
| 483-484 | `"The editor for what's next"` | 副标题 |
| 165-169 | `"Get Started"` → `"New File"`, `"Open Project"`, `"Clone Repository"`, `"Open Command Palette"` | 章节标题 + 条目 |
| 194-198 | `"Configure"` → `"Open Settings"`, `"Customize Keymaps"`, `"Explore Extensions"` | 章节标题 + 条目 |
| 333 | `"Run multiple threads at once, mix and match any ACP-compatible agent, and keep work conflict-free with worktrees."` | 代理卡片描述 |
| 354 | `"Collaborate with Agents"` | 代理卡片标签 |
| 363 | `"Open Agent Panel"` | 代理卡片按钮 |
| 384 | `"Recent Projects"` | 章节标题 |
| 500 | `"Return to Onboarding"` | 底部按钮 |
| 664 | `"Untitled"` | 回退项目名 |

### `crates/workspace/src/security_modal.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 82 | `"Unrecognized Project"` | 模态框标题（单数） |
| 84 | `"Unrecognized Projects ({count})"` | 模态框标题（复数） |
| 177 | `"Untrusted projects are opened in Restricted Mode to protect your system."` | 描述 |
| 183 | `"Review .zed/settings.json for any extensions or commands configured by this project."` | 描述 |
| 190 | `"Restricted Mode prevents:"` | 章节标题 |
| 191 | `"Project settings from being applied"` | 项目符号 |
| 192 | `"Language servers from running"` | 项目符号 |
| 193 | `"MCP Server integrations from installing"` | 项目符号 |
| 197-198 | `"Trust all projects in the {folder} folder"` | 复选框标签 |
| 217 | `"Stay in Restricted Mode"` | 按钮 |
| 232 | `"Trust and Continue"` | 按钮 |
| 284 | `"Trust all single files"` | 复选框标签 |
| 293 | `"Trust all projects in the parent folders"` | 复选框标签 |

### `crates/workspace/src/notifications.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 360-361 | `"Suppress"` / `"Click to close"` | 工具提示 |
| 368-369 | `"Close"` / `"Suppress with shift-click"` | 工具提示 |
| 993-1008 | `"Suppress"` / `"Click to Close"` / `"Shift-click to Suppress"` / `"Close"` | 工具提示 |
| 1190-1344 | `"Some informational content for the user."`, `"A new version of Zed is available..."`, `"Failed to save the file."` 等 | 通知组件预览字符串 |
| 1372-1392 | `"Header Actions (top right)"`, `"Close Only"`, `"Workspace Errors"` 等 | 预览章节标题 |
| 1663-1670 | `"OK"` | 提示按钮 |

### `crates/workspace/src/status_bar.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 229-231 | `"Open Threads Sidebar"` | 工具提示 |
| 279 | `"Hide Button"` | 右键菜单切换项 |

### `crates/workspace/src/dock.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 328 | `"Left"`, `"Bottom"`, `"Right"` | 停靠位置标签 |
| 1250 | `"Close {position} Dock"` | 面板按钮工具提示 |
| 1279 | `"Dock {position}"` | 上下文菜单位置条目 |
| 1300 | `"Flex Width"` | 上下文菜单切换项 |

### `crates/workspace/src/invalid_item_view.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 93 | `"Could not open file"` | 错误显示 |
| 102 | `"Open in Default App"` | 按钮 |

### `crates/workspace/src/workspace_error.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 84 | `"Dismiss"` | 默认错误操作标签 |
| 200-202 | `"See docs"` | 链接标签 |

### `crates/workspace/src/theme_preview.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 139-391 | `"Text"`, `"Headline Sizes"`, `"XLarge Headline"`, `"Text Colors"`, `"Colors"`, `"Wrapping Text"` 等 | 预览部分标题 |
| 360 | `"Theme Preview"` | 页面标题 |
| 361 | `"This view lets you preview a range of UI elements across a theme."` | 描述 |

### `crates/workspace/src/tasks.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 105 | `"Task spawn failed: {e}"` | Toast 信息 |

---

## 2. zed (app_menus)

### `crates/zed/src/zed/app_menus.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 63 | `"ZetaCode"` | 应用菜单名 |
| 66 | `"About ZetaCode"` | 关于 |
| 67 | `"Check for Updates"` | 检查更新 |
| 69 | `"Settings"` | 子菜单 |
| 70 | `"Open Settings"` | 设置 |
| 71-87 | `"Open Settings File"`, `"Open Project Settings"`, `"Select Theme..."`, `"Select Icon Theme..."` 等 | 设置子菜单 |
| 93 | `"Extensions"` | 扩展 |
| 95 | `"Install CLI"` | 安装 CLI |
| 98-104 | `"Hide ZetaCode"`, `"Hide Others"`, `"Show All"`, `"Quit ZetaCode"` | 应用菜单 |
| 108 | `"File"` | 菜单标题 |
| 111-141 | `"New"`, `"New Window"`, `"Open..."`, `"Open Recent..."`, `"Open Remote..."`, `"Save"`, `"Save As..."`, `"Save All"`, `"Close Editor"`, `"Close Window"` 等 | 文件菜单 |
| 145 | `"Edit"` | 菜单标题 |
| 148-162 | `"Undo"`, `"Redo"`, `"Cut"`, `"Copy"`, `"Paste"`, `"Find"`, `"Find in Project"`, `"Toggle Line Comment"` | 编辑菜单 |
| 166 | `"Selection"` | 菜单标题 |
| 169-210 | `"Select All"`, `"Expand Selection"`, `"Shrink Selection"`, `"Add Cursor Above/Below"`, `"Move Line Up/Down"`, `"Duplicate Selection"` 等 | 选择菜单 |
| 214 | `"View"` | 菜单标题 |
| 27-48 | `"Zoom In"`, `"Zoom Out"`, `"Toggle Left/Right/Bottom Dock"`, `"Editor Layout"`, `"Project Panel"`, `"Outline Panel"`, `"Diagnostics"` 等 | 视图菜单 |
| 219 | `"Go"` | 菜单标题 |
| 222-247 | `"Back"`, `"Forward"`, `"Command Palette..."`, `"Go to File..."`, `"Go to Definition"`, `"Find All References"`, `"Next Problem"` 等 | 导航菜单 |
| 251 | `"Run"` | 菜单标题 |
| 254-272 | `"Spawn Task"`, `"Start Debugger"`, `"Continue"`, `"Step Over/Into/Out"`, `"Toggle Breakpoint"` | 运行菜单 |
| 276 | `"Window"` | 菜单标题 |
| 279-280 | `"Minimize"`, `"Zoom"` | 窗口菜单 |
| 285 | `"Help"` | 菜单标题 |
| 288-318 | `"View Release Notes Locally"`, `"File Bug Report..."`, `"Request Feature..."`, `"Documentation"`, `"Zed Repository"`, `"Zed Twitter"`, `"Join the Team"` 等 | 帮助菜单 |

---

## 3. editor

### `crates/editor/src/actions.rs`

~120 个动作描述注释，出现在命令面板中:

| 字符串 | 动作 |
|---------|------|
| `"Selects the next occurrence of the current selection."` | SelectNext |
| `"Moves the cursor to the beginning of the current line."` | MoveToBeginningOfLine |
| `"Toggles comment markers for the selected lines."` | ToggleComments |
| `"Finds all references to the symbol at cursor."` | FindAllReferences |
| `"Renames the symbol at cursor."` | Rename |
| `"Formats the entire document."` | Format |
| `"Toggles inline git blame display."` | ToggleGitBlame |
| ...（~120 个类似条目） | 各种编辑器动作 |

详细列表见 `crates/editor/src/actions.rs`。

### `crates/editor/src/editor.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 2013 | `"Rename"` | 重命名事务标签 |
| 2751 | `"Failed to create buffer"` | 错误对话框标题 |
| 2756 | `"The remote instance of Zed does not support this yet. It must be upgraded to {}"` | 错误详情 |
| 3968-3978 | `"Remove Bookmark"` / `"Right-click for more options"` | 工具提示 |
| 4064-4122 | `"Edit Log Breakpoint"` / `"Set Breakpoint"` / `"Disable"` / `"Run to Cursor"` | 上下文菜单 |
| 4286-4384 | `"Unset breakpoint"` / `"Right-click for more options"` / `"Set bookmark"` | 工具提示 |
| 12070-12072 | `"Message to log when a breakpoint is hit..."` / `"Condition when a breakpoint is hit..."` | 占位符文本 |
| 12175-12188 | `"Cancel"` / `"Confirm"` | 按钮工具提示 |

### `crates/editor/src/element.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 6712 | `"›"` | 面包屑分隔符 |
| 6756 | `"Show Symbol Outline"` | 面包屑工具提示 |
| 6771 | `"Right-Click to Copy Path"` | 面包屑工具提示 |

### `crates/editor/src/element/header.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 803 | `"untitled"` | 回退文件名 |
| 896 | `"Open File"` | 按钮标签 |
| 985 | `"Copy Path"` | 上下文菜单 |
| 996 | `"Copy Relative Path"` | 上下文菜单 |
| 1011 | `"Reveal In Project Panel"` | 上下文菜单 |
| 1023 | `"Open in Terminal"` | 上下文菜单 |

### `crates/editor/src/mouse_context_menu.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 250-312 | `"Run to Cursor"`, `"Go to Definition"`, `"Go to Declaration"`, `"Find All References"`, `"Rename Symbol"`, `"Format Buffer"`, `"Show Code Actions"`, `"Cut"`, `"Copy"`, `"Paste"`, `"Open in Terminal"`, `"Copy Permalink"`, `"View File History"` 等 | 鼠标上下文菜单 |

### `crates/editor/src/hover_popover.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 1239 | `"Copy Diagnostic"` | 复制按钮工具提示 |

### `crates/editor/src/git.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 442 | `"Add a review comment..."` | 占位符文本 |
| 831 | `"Add Review (drag to select multiple lines)"` | 工具提示 |
| 2244 | `"Close"` | 按钮工具提示 |
| 2254 | `"Add comment"` | 按钮工具提示 |
| 2322 | `"{} Comment{}"` | 评论章节标题 |
| 2749 | `"Next Hunk"` | 下一个差异块按钮 |
| 2782 | `"Previous Hunk"` | 上一个差异块按钮 |

### `crates/editor/src/code_context_menus.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 1611-1636 | `"Text"`, `"Method"`, `"Function"`, `"Constructor"`, `"Field"`, `"Variable"`, `"Class"`, `"Interface"`, `"Module"`, `"Property"`, `"Value"`, `"Enum"`, `"Keyword"`, `"Snippet"`, `"File"`, `"Reference"`, `"Folder"`, `"Constant"`, `"Struct"`, `"Unknown"` | 完成类型徽章工具提示 |

### `crates/editor/src/code_actions.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 299 | `"Toggle Code Actions"` | 代码操作指示器工具提示 |

### `crates/editor/src/edit_prediction.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 1154 | `"Accept"` | 行末弹窗标签 |
| 1278 | `"Hold"` | 光标弹窗标签 |
| 1398 | `"Preview"` | 预览标签 |
| 1830-1867 | `"Jump to Edit"` | 跳转弹窗标签 |
| 2295 | `"untitled"` | 回退文件名 |
| 2439 | `"Jump to {file_name}"` | 跳转到外部文件 |
| 2477 | `"…"` | 更多指示符 |
| 2524-2539 | `"Conflict with Accept Keybinding"` / `"Assign Keybinding"` / `"See Docs"` | 快捷键冲突工具提示 |

---

## 4. project_panel

### `crates/project_panel/src/project_panel.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 297-300 | `"this is a bug..."` / `"Undo failed"` / `"Redo failed"` | 错误标题 |
| 916-926 | `"Failed to open file"`, `"Disconnected from SSH host"`, `"Disconnected from remote project"` | 错误提示 |
| 1063-1220 | `"Search Inside"`, `"New File"`, `"New Folder"`, `"Reveal in Finder"`, `"Open in Default App"`, `"Open in Terminal"`, `"Find in Folder…"`, `"Fold Directory"`, `"Cut"`, `"Copy"`, `"Duplicate"`, `"Paste"`, `"Undo"`, `"Redo"`, `"Copy Path"`, `"Copy Relative Path"`, `"Rename"`, `"Trash"`, `"Delete"`, `"Add Folders to Project…"`, `"Remove from Project"`, `"Collapse All"`, `"Restore File"`, `"Add to .gitignore"`, `"Add to .git/info/exclude"`, `"View History"`, `"Download..."`, `"Compare Marked Files"` | 上下文菜单 |
| 1719-1769 | `"File or directory name cannot be empty."`, `"File or directory name contains leading or trailing whitespace."`, `"File or directory '{}' already exists at location."` | 验证错误/警告 |
| 1926-1933 | `"Created an excluded directory at {}. Alter `file_scan_exclusions` in the settings..."` | Toast 信息 |
| 2249-2271 | `"Discard changes to {}?"`, `"Restore"`, `"Cancel"`, `"Failed to restore {}: {}"` | 恢复文件提示 |
| 2442-2505 | `"Trash"`/`"Delete"`, `"Do you want to trash"`, `"Are you sure you want to permanently delete"`, `"This cannot be undone."`, `"Cancel"` | 删除提示 |
| 2472-2490 | `".. 1 file not shown"`, `".. {} files not shown"`, `"{} of these have unsaved changes, which will be lost."` | 截断文件列表 |
| 3422-3475 | `"Downloading 0/{} files..."`, `"Downloading {}/{} files..."`, `"Downloaded {} files"` | 下载进度 |
| 5855 | `"Symbolic Link"` | 符号链接工具提示 |
| 7218 | `"Project Panel"` | 空状态标题 |
| 7341 | `"Project Panel"` | 面板图标工具提示 |
| 7454-7475 | `"!"`, `"U"`, `"D"`, `"M"`, `"A"` | Git 状态指示器 |

---

## 5. git_ui

### `crates/git_ui/src/git_panel.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 167-229 | `"Stage All"`, `"Unstage All"`, `"Stash All"`, `"Stash Pop"`, `"View Stash"`, `"Open Diff"`, `"Discard Tracked Changes"`, `"Trash Untracked Files"`, `"Flat View"` / `"Tree View"`, `"Sort by Status"` / `"Sort by Path"` | 面板上下文菜单 |
| 4484-4514 | `"You must resolve conflicts before committing"`, `"No changes to commit"`, `"Commit in progress"`, `"No commit message"`, `"You do not have write access to this project"` | 提交按钮工具提示 |
| 4503-4512 | `"Amend"`, `"Amend Tracked"`, `"Commit"`, `"Commit Tracked"` | 提交按钮标题 |
| 4448-4477 | `"Amend"`, `"Signoff"` | 提交菜单切换项 |
| 5133-5148 | `"Changes"`, `"History"` | 面板选项卡标签 |
| 5167-5185 | `"Loading Commit History…"`, `"No commits yet"`, `"Failed to load commits"` | 历史选项卡状态 |
| 5595-5668 | `"No changes to commit"`, `"View Branch Diff"`, `"No Git Repositories"`, `"Initialize Repository"` | 空/未初始化状态 |
| 5620-5650 | `"Detected dubious ownership in repository at {}."`, `"Trust Directory"`, `"Learn More"` | 不安全仓库信息 |
| 6014-6048 | `"Unstage File"`, `"Stage File"`, `"Trash File"`, `"Discard Changes"`, `"Add to .gitignore"`, `"Open Diff"`, `"View File History"` | 文件上下文菜单 |
| 7191 | `"/"` | 路径分隔符 |
| 7481 | `"Output from git {}"` | 输出缓冲区标题 |
| 7507 | `"View Log"` | Toast 操作按钮 |
| 4175-4182 | `"Create Pull Request"`, `"View Log"` | Toast 操作按钮 |
| 4307-4370 | `"Generate Commit Message"`, `"Generating Commit…"`, `"No Changes to Commit"`, `"Configure an LLM provider to generate commit messages"` | 提交信息生成 |
| 4969-4975 | `"This will update your most recent commit."`, `"Cancel"` | 修改待定标签 |

### `crates/git_ui/src/git_ui.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 669 | `"View Commit"` | 模态框标题 |
| 743-873 | `"Fetch"`, `"Push"`, `"Pull"`, `"Publish"`, `"Republish"` + 工具提示 | 远程按钮 |
| 909-922 | `"Fetch"`, `"Fetch From"`, `"Pull"`, `"Pull (Rebase)"`, `"Push"`, `"Push To"`, `"Force Push"` | 远程操作上下文菜单 |
| 1137 | `"Clone a repository from GitHub or other sources."` | 克隆模态框描述 |
| 1142 | `"Learn More"` | 克隆模态框按钮 |

### `crates/git_ui/src/branch_picker.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 690-692 | `"Branch \"{branch_name}\" is not fully merged. Force delete it?"` | 强制删除提示 |
| 745 | `"Force Delete Branch"` | 工具提示 |
| 1088 | `"Some branches could not be loaded: {error}"` | 错误横幅 |
| 1456-1462 | `"Create Remote Repository"`, `"Create Branch: \"{name}\"…"`, `"Create Remote: \"{name}\""` | 列表条目 |
| 1525-1528 | `"Create New From: {default_branch}"` | 按钮 + 工具提示 |
| 1636 | `"No commits found"` | 空状态 |
| 1663 | `"Selected Branch"` | 标签 |
| 1670 | `"Current Branch"` | 标签 |
| 1739-1892 | `"Create New From:"`, `"Delete"`, `"Switch"`, `"Create"`, `"Confirm"` | 页脚按钮 |
| 1818-1819 | `"Filter Remote"` / `"Show All"` | 过滤器切换按钮 |

### `crates/git_ui/src/commit_modal.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 370 | `"<no branch>"` | 回退分支名 |
| 397 | `"Switch Branch"` | 工具提示 |
| 407 | `"Cancel"` | 快捷键提示后缀 |
| 450-485 | `commit_label` (dynamic `"Commit"/"Amend"/etc.`), `"--amend"`, `"--signoff"` | 提交按钮标签 |

### `crates/git_ui/src/commit_view.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 603-605 | `"Fold Commit Description"` / `"Expand Commit Description"` | 工具提示 |
| 663 | `"•"` | 分隔点 |
| 678 | `"Commit SHA"` | 按钮标签 |
| 688 | `"Copy Commit SHA"` | 工具提示 |
| 1312 | `"Buffer Search"` | 工具提示 |
| 1328 | `"Show in Git Graph"` | 工具提示 |
| 1341 | `"View on {}"` | 工具提示 |

### `crates/git_ui/src/conflict_view.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 333-393 | `"Use {}"`, `"Use Both"`, `"Resolve with Agent"` | 冲突解决按钮 |
| 612-622 | `"Resolve Merge Conflict{} with Agent"`, `"Found {} {} across the codebase"` | 指示器信息 |

### `crates/git_ui/src/git_graph.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 3788-3817 | `"Graph"`, `"Description"`, `"Date"`, `"Author"`, `"Commit"` | 列标题 |
| 2482-2570 | `"View Commit"`, `"Copy SHA"`, `"Copy Ref Name"`, `"Copy Tag"`, `"Custom Commands"`, `"Learn More"` | 上下文菜单 |
| 2631-2717 | `"Select Previous Match"`, `"Select Next Match"` | 搜索栏工具提示 |
| 2960-3155 | `"Email Copied!"`, `"Copy Email"`, `"Commit SHA Copied!"`, `"Copy Commit SHA"`, `"View on {}"`, `"{} Changed {}"`, `"Show Flat View"`, `"Show Tree View"` | 详情面板 |
| 399 | `"Toggle Folder"` | 文件夹切换工具提示 |

### `crates/git_ui/src/stash_picker.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 272 | `"#{}: {}"` | 暂存列表项格式 |
| 518 | `"•"` | 分隔点 |
| 533-557 | `"View Stash"`, `"Pop Stash"`, `"Drop Stash"` | 工具提示 |
| 640-680 | `"Drop"`, `"View"`, `"Pop"`, `"Apply"` | 页脚按钮 |

### `crates/git_ui/src/solo_diff_view.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 553 | `"Unified"` | 差异样式工具提示 |
| 560 | `"Split"` | 差异样式工具提示 |
| 668-780 | `"Toggle Staged"`, `"Stage"`, `"Unstage"`, `"Restore"`, `"Go to previous hunk"`, `"Go to next hunk"`, `"Stage File"`, `"Unstage File"`, `"Commit"` | Git 工具栏 |

### `crates/git_ui/src/project_diff.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 1166-1167 | `"Uncommitted Changes"`, `"Changes since {}"` | 选项卡文本 |
| 1362-1380 | `"No uncommitted changes"`, `"Remote up to date"`, `"Close"` | 空状态 |
| 1646-1798 | `"Toggle Staged"`, `"Stage"`, `"Unstage"`, `"Go to previous/next hunk"`, `"Stage All"`, `"Unstage All"`, `"Commit"`, `"Send Review to Agent ({})"`, `"Send all review comments to the Agent panel"` | 工具栏 |
| 1860-1948 | `"Base: {base_ref}"`, `"Select base branch"`, `"Review Diff"` | 分支差异工具栏 |

### `crates/git_ui/src/blame_ui.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 155-159 | `"{author}, {relative_timestamp} - {summary}"` | 内联责备条目 |
| 366 | `"Copy SHA"` | 工具提示 |
| 408-414 | `"Copy Commit SHA"`, `"Open Permalink"` | 上下文菜单 |

### `crates/git_ui/src/remote_output.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 44-128 | `"Synchronized with remotes"`, `"Fast forwarded from {}"`, `"Successfully pulled from {}"`, `"Pushed {} to {}"` 等 | 远程操作成功信息 |

### `crates/git_ui/src/worktree_picker.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 358 | `"Worktree \"{display_name}\" contains modified or untracked files. Force delete it?"` | 强制删除提示 |
| 1108-1128 | `"current branches"`, `"HEAD"`, `"Create new worktree based on {}"` | 列表条目 |
| 1151 | `"Deleting…"` | 状态标签 |
| 1258-1315 | `"Open in New Window"`, `"Force Delete Branch"`, `"Remove Worktree from Window"` | 工具提示 |
| 1347 | `"HEAD"` | 回退 |
| 1351 | `"Create \"{name}\" based on {}"` | 列表条目 |
| 1418-1441 | `"Create"`, `"Deleting…"`, `"Delete"` | 页脚按钮 |

### `crates/git_ui/src/askpass_modal.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 93 | `"You may need to configure git for Github."` | 提示标签 |
| 96 | `"Learn more"` | 按钮 |

---

## 6. collab_ui

### `crates/collab_ui/src/collab_panel.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 2643 | `"Work with your team in realtime with collaborative editing, voice, shared notes and more."` | 未登录信息 |
| 2654-2664 | `"Connecting…"`, `"Connect"`, `"Signing in…"`, `"Sign In with GitHub"` | 按钮 |
| 2879-2902 | `"Copy public channel link."`, `"Current Call"`, `"Favorites"`, `"Requests"`, `"Contacts"`, `"Channels"`, `"Invites"`, `"Online"`, `"Offline"` | 部分标题/工具提示 |
| 2926-3007 | `"Copy Channel Link"`, `"Auto Watch Screens"`, `"Search for new contact"`, `"Show All Channels"`, `"Create Channel"` | 工具提示 |
| 1126-1402 | `"Follow {}"`, `"Leave Call"`, `"Click to Follow"`, `"untitled"`, `"Failed to join project"`, `"Screen"`, `"notes"`, `"Grant Mic Access"`, `"Grant Write Access"` | 通话参与者控制 |
| 1411-1677 | `"Mute"`, `"Revoke Access"`, `"Expand Subchannels"`, `"Open Notes"`, `"Copy Channel Link"`, `"Remove from Favorites"`, `"Rename"`, `"Delete"`, `"Leave Channel"`, `"Call {}"`, `"Remove Contact"` | 上下文菜单 |
| 1897-2602 | `"Failed to create channel"`, `"Failed to hang up"`, `"Are you sure you want to leave #{}?"`, `"Leave"`, `"Cancel"`, `"Remove"`, `"Call failed"` | 错误/确认对话框 |
| 2810 | `"Clear Filter"` | 工具提示 |
| 3108-3162 | `"{} is offline"`, `"Invite {} to join call"`, `"Decline invite"`, `"Accept invite"`, `"Cancel invite"` | 联系人条目工具提示 |
| 3463 | `"Open Channel Notes"` | 工具提示 |
| 3535-3554 | `"{} wants to add you as a contact"`, `"{} accepted your contact request"`, `"{} invited you to join the #{} channel"` | 通知文本 |
| 3820 | `"Collab Panel"` | 图标工具提示 |
| 3995 | `"Join Channel"` | 工具提示 |
| 4058-4074 | `"Accept"`, `"Dismiss"`, `"Decline"`, `"Close"` | Toast 按钮 |

### `crates/collab_ui/src/call_stats_modal.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 80-86 | `"Excellent"`, `"Good"`, `"Poor"`, `"Lost"`, `"—"` | 质量标签 |
| 92-131 | `"Normal"`, `"High"`, `"Poor"` | 指标评级 |
| 160 | `"Unable to fetch call statistics"` | 错误标签 |
| 179 | `"Call Diagnostics"` | 模态框标题 |
| 191 | `"Not in a call"` | 状态标签 |
| 201-228 | `"Network"`, `"Latency"`, `"Jitter"`, `"Packet loss"`, `"Input lag"` + 描述 | 指标标题 |

### `crates/collab_ui/src/collab_panel/channel_modal.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 182 | `"Copy Link"` | 按钮 |
| 207-222 | `"Manage Members"`, `"Invite Members"` | 选项卡标签 |
| 262 | `"Search collaborator by username..."` | 占位符文本 |
| 429-460 | `"Invited"`, `"Admin"`, `"Guest"`, `"You"`, `"Member"` | 角色/状态标签 |
| 626-662 | `"Demote to Guest"`, `"Promote to Admin"`, `"Remove from Channel"` | 上下文菜单 |

### `crates/collab_ui/src/collab_panel/contact_finder.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 47-92 | `"Contacts"`, `"Invite new contacts"`, `"Search collaborator by username..."` | 标题/占位符 |

### `crates/collab_ui/src/channel_view.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 211 | `"Copy Link to Section"` | 上下文菜单 |

### `crates/collab_ui/src/notifications/`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 119-129 | `"Accept"`, `"Decline"`, `"{} is sharing a project in Zed"` | 通话通知 |
| 128-139 | `"{} is sharing a project with you"`, `"Open"`, `"Dismiss"` | 项目共享通知 |

---

## 7. terminal_view

### `crates/terminal_view/src/terminal_panel.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 161 | `"New…"` | "+" 按钮工具提示 |
| 170-177 | `"New Terminal"`, `"Spawn Task"` | 菜单动作 |
| 192 | `"Split Pane"` | 工具提示 |
| 202-205 | `"Split Right"`, `"Split Left"`, `"Split Up"`, `"Split Down"` | 菜单动作 |
| 222-223 | `"Zoom Out"` / `"Zoom In"` | 缩放工具提示 |
| 763-877 | `"terminal not yet supported for remote projects"`, `"terminal not yet supported for collaborative projects"` | 错误信息 |
| 1309-1312 | `"Open Settings"`, `"Edit settings.json"` | 菜单动作 |
| 1336-1361 | `"Failed to spawn terminal"`, `"Edit Settings"` | 错误视图 |
| 1654 | `"Terminal Panel"` | 图标工具提示 |
| 1720 | `"Inline Assist"` | 工具提示 |

### `crates/terminal_view/src/terminal_view.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 509-533 | `"New Terminal"`, `"New Center Terminal"`, `"Copy"`, `"Paste"`, `"Select All"`, `"Clear"`, `"Inline Assist"`, `"Add to Agent Thread"`, `"Close Terminal Tab"` | 上下文菜单 |
| 1058 | `"Rerun task"` | 工具提示 |
| 1417 | `"Process ID (PID): {}"` | 选项卡工具提示 |
| 1696 | `"Rename"` | 额外选项卡菜单 |

---

## 8. settings_ui

### `crates/settings_ui/src/settings_ui.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 1647 | `"Search settings…"` | 搜索占位符 |
| 2862-2864 | `"Focus Content"`, `"Focus Navbar"` | 导航栏焦点模式 |
| 3362-3364 | `"No Results"`, `"No settings match \"{}\""` | 空搜索结果 |
| 492 | `"Edit in settings.json"` | 按钮 |
| 2722 | `"Edit in settings.json"` | 文件头操作按钮 |
| 1425 | `"Copy Link"` | 复制链接工具提示 |
| 2759 | `"View Other Projects"` | 文件溢出工具提示 |
| 3285-3315 | `"Scope"`, `"Change Scope"` | 作用域选择器 |
| 3334 | `"/"` | 面包屑分隔符 |
| 3589-3691 | `"Edit in settings.json"`, `"Fix in settings.json"`, `"Your settings are out of date..."`, `"Failed to load your settings..."`, `"Your settings file is out of date..."` | 横幅文本 |
| 3604 | `"Create Skill"` | 按钮 |
| 3673 | `"Failed to load your settings. Some values may be incorrect and changes may be lost."` | 解析错误横幅 |
| 3723-3734 | `"Restricted Mode"`, `"This project is in restricted mode."`, `"Manage Trust"` | 受限模式横幅 |

### `crates/settings_ui/src/page_data.rs`

~200+ 设置页面标题、章节标题和设置项标题/描述。包括:

| 字符串 | 类型 |
|---------|------|
| `"General"`, `"Appearance"`, `"Keymap"`, `"Editor"`, `"Languages & Tools"`, `"Search & Files"`, `"Window & Layout"`, `"Panels"`, `"Debugger"`, `"Terminal"`, `"Version Control"`, `"Collaboration"`, `"AI"`, `"Network"` | 页面标题 |
| `"Theme"`, `"Buffer Font"`, `"UI Font"`, `"Auto Save"`, `"Gutter"`, `"Minimap"`, `"Scrollbar"`, `"Toolbar"`, `"Vim"`, `"Search"`, `"File Finder"`, `"Project Panel"`, `"Git Panel"`, `"Agent Panel"` | 章节标题 |
| `"Theme Mode — Choose a static, fixed theme..."`, `"Font Family — Font family for editor text."`, `"Font Size — Font size for editor text."`, `"Line Height — Line height for editor text."` | 设置项标题/描述 |
| `"When Closing With No Tabs"`, `"On Last Window Closed"`, `"Restore Unsaved Buffers"`, `"Telemetry Diagnostics"`, `"Proxy"`, `"Server URL"` | 设置项标题 |

详情见 `crates/settings_ui/src/page_data.rs`。

### `crates/settings_ui/src/components/`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| L257-276 | `"Clear"`, `"Enter to Confirm"` | 输入字段工具提示 |
| L74 | `"Search fonts…"` | 字体选择器占位符 |
| L82 | `"Search icon themes…"` | 图标主题选择器占位符 |
| L54 | `"Overridden by dev extension."` | 扩展卡片覆盖文本 |

### `crates/settings_ui/src/pages/skill_creator.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 186-838 | `"Name"`, `"Add skill content…"`, `"Import from URL"`, `"(optional)"`, `"Fetching and parsing…"`, `"Front-matter"`, `"Skill Content"`, `"Save Skill"` / `"Saving…"` | 表单标签/按钮/占位符 |

### `crates/settings_ui/src/pages/`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 72 | `"Create a Skill"` | 技能页面按钮 |
| 137-224 | `"Stop Testing"` / `"Start Testing"`, `"Output Device"`, `"Input Device"` | 音频测试窗口 |
| 63 | `"enabled for all"` | 功能标记行标签 |
| 382-1142 | `"Dismiss"`, `"Test Your Rules"`, `"Default Permission"`, `"Default Action"` 等 | 工具权限设置 |
| 154-299 | `"Provider"`, `"Select which provider to use for edit predictions."`, `"Visit the"`, `"to generate an API key."`, `"API Key"` | 编辑预测提供者设置 |

---

## 9. extensions_ui

### `crates/extensions_ui/src/extensions_ui.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 356 | `"Search extensions..."` | 搜索占位符 |
| 1658-1807 | `"Extensions"` | 页面标题/选项卡内容 |
| 1680-1729 | `"All"`, `"Installed"`, `"Not Installed"`, `"All"` | 过滤器切换/分类 |
| 1341-1364 | `"Loading extensions…"`, `"Failed to load extensions. Please check your connection..."`, `"No extensions that match your search."`, `"No extensions."`, `"No installed extensions."`, `"No not installed extensions."` | 空/加载/错误状态 |
| 657 | `"Rebuild"` | 开发扩展重建按钮 |
| 946-961 | `"Install Another Version..."`, `"Copy Extension ID"`, `"Copy Author Info"` | 上下文菜单 |
| 1026-1190 | `"Install"`, `"Uninstall"`, `"Upgrade"`, `"Configure"` | 扩展操作按钮 |
| 1453 | `"View Documentation"` | 文档按钮 |
| 1484 | `"Enable Vim mode"` | 升级销售横幅标签 |
| 1877-1897 | `"Themes"`, `"Icon Themes"`, `"Languages"`, `"Language Servers"`, `"MCP Servers"`, `"Slash Commands"`, `"Indexed Docs Providers"` 等 | 扩展提供标签 |
| 1149 | `"v{version} is not compatible with this version of Zed."` | 不兼容工具提示 |

### `crates/extensions_ui/src/extension_suggest.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 175-193 | `"Do you want to install the recommended '{}' extension for '{}' files?"`, `"Yes, install extension"`, `"No, don't install it"` | 通知 |

### `crates/extensions_ui/src/extension_version_selector.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 96 | `"Select extension version..."` | 选择器占位符 |
| 237 | `"Incompatible"` | 版本兼容性标签 |

---

## 10. command_palette / file_finder / search

### `crates/command_palette/src/command_palette.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 379-381 | `"Execute a command..."` | 占位符文本 |
| 653-662 | `"Change Keybinding…"`, `"Add Keybinding…"` | 页脚按钮 |
| 683 | `"Run"` | 页脚按钮 |

### `crates/file_finder/src/file_finder.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 1677-1678 | `"Search project files..."` | 占位符文本 |
| 1918 | `"Project Scan in Progress…"` | 扫描指示器工具提示 |
| 1960-1977 | `"Filter Options"`, `"Include Ignored Files"` | 过滤器触发/上下文菜单 |
| 2010-2065 | `"Split…"`, `"Split Left/Right/Up/Down"`, `"Keep Open"`, `"Open"` | 按钮/上下文菜单 |
| 1295-1298 | `"Channel Notes"`, `"Create File: {}"` | 结果副标题 |

### `crates/search/src/search.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 98-107 | `"Match Whole Words"`, `"Match Case Sensitivity"`, `"Also search files ignored by configuration"`, `"Use Regular Expressions"`, `"One Match Per Line"`, `"Search Backwards"` | 搜索选项标签 |
| 199 | `"No more matches"` | Toast 通知 |

### `crates/search/src/buffer_search.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 303-308 | `"Search…"`, `"Replace with…"` | 占位符文本 |
| 137-247 | `"Unified"`, `"Split"`, `"Expand All Files"`, `"Collapse All Files"` | 工具提示 |
| 325-331 | `"{}/{}"`, `"0/0"` | 匹配计数器 |
| 396-572 | `"Toggle Replace"`, `"Toggle Search Selection"`, `"Select Previous Match"`, `"Select All Matches"`, `"Close Search Bar"`, `"Replace Next Match"`, `"Replace All Matches"` | 工具提示 |

### `crates/search/src/project_search.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 567-570 | `"Loading project…"`, `"Searching…"`, `"No Results"`, `"Search All Files"` | 标题 |
| 580 | `"No results found in this project for the provided query"` | 无结果副标题 |
| 667 | `"Project Search"` | 选项卡内容 |
| 985-1053 | `"Search all files…"`, `"Replace in project…"`, `"Include:"`, `"Exclude:"` | 占位符 |
| 1330-1333 | `"Save"` / `"Don't Save"` / `"Cancel"`, `"Project search buffer contains unsaved edits..."` | 对话框 |
| 1733-1774 | `"Hit enter to search. For more options:"`, `"Match whole words"`, `"Match case"` | 登录页面 |
| 2231-2474 | `"{index}/{match_quantity}"`, `"Select Previous Match"`, `"Search Limits Reached"`, `"Toggle Filters"`, `"Toggle Replace"`, `"Only Search Open Files"` | 计数器/工具提示 |

### `crates/search/src/search_status_button.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 36-43 | `"Project Search"` | 状态栏按钮工具提示 |

---

## 11. agent_ui

> agent_ui 是字符串最多的 crate。以下列出主要类别，完整逐行详情见源码文件。

### `crates/agent_ui/src/conversation_view/thread_view.rs`

**工具提示 / 按钮:**
`"Copy Code"`, `"Review"`, `"Reject"`, `"Keep"`, `"Stop Subagent"`, `"Minimize Subagent"`, `"Edit"`, `"Send Now"`, `"Edit Queued Message"`, `"Remove Message from Queue"`, `"Disable Fast Mode"` / `"Enable Fast Mode"`, `"Select Effort"`, `"Change Thinking Effort"`, `"Restore Checkpoint"`, `"Open Thread as Markdown"`, `"Scroll To Most Recent User Prompt"`, `"Scroll To Top"`, `"Sync with source thread"`, `"Clear All"`, `"Clear Plan"`, `"Reject All"`, `"Keep All"`, `"Review Changes"`, `"Scroll to Subagent"`, `"Thanks for your feedback!"`, `"Helpful Response"`

**标注 / 标签:**
`"Plan"`, `"Completed Plan"`, `"Edits"`, `"Write access"`, `"Subagent"`, `"Subagents Awaiting Permission:"`, `"Current:"`, `"Awaiting Confirmation"`, `"Thinking"`, `"Everything below this line was sent as output from this subagent…"`

**Callout 标题/描述:**
`"Authentication Required"`, `"Free Usage Exceeded"`, `"Context Too Large"`, `"Thread reaching the token limit soon"`, `"Thread reached the token limit"`, `"To continue, start a new thread from a summary."`, `"The model"`, `"Start New Thread"`, `"Learn More"`, `"Switch to {}"`, `"Accept"`, `"Dismiss"`, `"Open Skill"`, `"Skill Failed to Load"`, `"Resumed Session"`, `"Review Before Sending"`

**进度 / 状态:**
`"Compacting Context…"`, `"Context Compacted"`, `"Compaction Canceled"`, `"Loading Added Context…"`, `"Editing {} {}…"`, `"Awaiting Confirmation ({pending_count})"`, `"{} tokens"`, `"Output exceeded terminal max lines and was truncated..."`

### `crates/agent_ui/src/agent_configuration/`

**添加 LLM 提供者模态框:**
`"Provider Name"`, `"API URL"`, `"API Key"`, `"Model Name"`, `"e.g. gpt-5, claude-opus-4, gemini-2.5-pro"`, `"Max Completion Tokens"`, `"Supports tools"`, `"Supports images"`, `"Add Model"`, `"Remove Model"`, `"Add LLM Provider"`, `"Cancel"`, `"Save Provider"`, `"Model Name cannot be empty"`, `"API Key cannot be empty"`

**配置 MCP 服务器模态框:**
`"Add MCP Server"`, `"Configure MCP Server"`, `"Cancel"`, `"Dismiss"`, `"Add Server"`, `"Authenticate"`, `"Submit"`, `"Open Repository"`, `"Authenticating…"`, `"Context server stopped running"`

**管理配置文件模态框:**
`"Customize"`, `"Custom Profiles"`, `"Add New Profile"`, `"Unknown"`, `"New Profile"`, `"Fork Profile"`, `"Configure Default Model"`, `"Configure Built-in Tools"`, `"Configure MCP Tools"`, `"Delete Profile"`, `"Go Back"`

### `crates/agent_ui/src/agent_configuration.rs`

`"Add Provider"`, `"Remove Provider"`, `"Add Server"`, `"Start New Thread"`, `"Log Out"`, `"Authenticate"`, `"Add Agent"`, `"Restart Agent Connection"`, `"Remove Registry Agent"`, `"Remove Custom Agent"`, `"1 tool"` / `"{} tools"`, `"Configure MCP Server"`

### `crates/agent_ui/src/agent_diff.rs`

`"Review: {title}"` / `"Review"`, `"No changes to review"`, `"Next Hunk"`, `"Previous Hunk"`, `"Generating Changes…"`, `"Review All Files"`

### `crates/agent_ui/src/inline_prompt_editor.rs`

`"Add Context"`, `"Or type @ to include context"`, `"Good Result"`, `"Bad Result"`, `"Execute Generated Command"`, `"Close Assistant"`, `"Previous Alternative"`, `"Next Alternative"`, `"{}/{}"`

### `crates/agent_ui/src/agent_panel.rs`

`"Terminal"`, `"Title generation failed. Retry"`, `"Edit Thread Title"`, `"Go Back"`, `"New {} Thread"`

### `crates/agent_ui/src/thread_import.rs`

`"Import External Agent Threads"`, `"No external agents available."`, `"Fetching Agent Threads…"`, `"Import Threads"`, `"No threads"` / `"{} threads"`

### `crates/agent_ui/src/agent_registry_ui.rs`

`"Visit Agent Repository"`, `"Visit Agent Website"`, `"ACP Registry"`

### `crates/agent_ui/src/ui/agent_notification.rs`

`"View"`, `"Dismiss"`

### `crates/agent_ui/src/ui/end_trial_upsell.rs`

`"Pro"`, `"Upgrade to Zed Pro"`, `"Free"`, `"(Current Plan)"`, `"Your Zed Pro Trial has expired"`, `"You've been automatically reset to the Free plan."`

### `crates/agent_ui/src/ui/model_selector_components.rs`

`"Configure"`, `"Change Model"`, `"Cycle Favorite Models"`, `"Latest"`

### `crates/agent_ui/src/mode_selector.rs`

`"Change Mode"`, `"Cycle Through Modes"`

---

## 12. diagnostics

### `crates/diagnostics/src/diagnostics.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 107 | `"No problems in workspace"` | 空状态标签 |
| 109 | `"No errors in workspace"` | 空状态标签 |
| 127 | `"Show {} warning{}"` | 显示警告按钮 |
| 769 | `"No problems"` | 选项卡内容标签 |

### `crates/diagnostics/src/buffer_diagnostics.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 900-901 | `"No problems in"` / `"No errors in"` | 空状态标签 |
| 919 | `"Open File"` | 文件打开按钮工具提示 |
| 939-940 | `"Show 1 warning"` / `"Show {} warnings"` | 显示警告按钮 |

### `crates/diagnostics/src/toolbar_controls.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 55 | `"Exclude Warnings"` / `"Include Warnings"` | 警告切换工具提示 |
| 65 | `"Buffer Search"` | 搜索按钮工具提示 |
| 77 | `"Inline Assist"` | 内联助手工具提示 |
| 92 | `"Stop Diagnostics Update"` | 停止按钮工具提示 |
| 107 | `"Refresh Diagnostics"` | 刷新按钮工具提示 |

### `crates/diagnostics/src/items.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 69 | `"Expand Diagnostics"` | 工具提示 |
| 71 | `"Next Diagnostic"` | 工具提示 |
| 97 | `"Project Diagnostics"` | 指示器工具提示 |

---

## 13. outline_panel

### `crates/outline_panel/src/outline_panel.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 460 | `"…"` | 截断上下文标记 |
| 1456-1468 | `"Open in Terminal"`, `"Unfold Directory"`, `"Fold Directory"`, `"Copy Path"`, `"Copy Relative Path"` | 上下文菜单 |
| 2425-2427 | `"Untitled"`, `"Unknown buffer"` | 回退名 |
| 4625-4650 | `"No matches for query"`, `"No outlines available"`, `"Toggle Panel With"` | 空状态 |
| 4820-4850 | `"Unpin Outline"`, `"Pin Active Outline"`, `"Clear Filter"` | 工具提示 |
| 4958-4997 | `"Outline Panel"` | 面板名/图标工具提示 |

---

## 14. tasks_ui

### `crates/tasks_ui/src/modal.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 58-60 | `"Find a task, or run a command in the central pane"` / `"Find a task, or run a command"` | 占位符文本 |
| 569 | `"Delete from Recent Tasks"` | 工具提示 |
| 646-707 | `"Rerun Last Task"`, `"Spawn Oneshot"`, `"Rerun Without History"`, `"Spawn Without History"`, `"Rerun"`, `"Spawn"` | 页脚按钮 |

---

## 15. onboarding

### `crates/onboarding/src/onboarding.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 352-361 | `"Welcome to ZetaCode"`, `"The editor for what's next"` | 标题/副标题 |
| 364-374 | `"Finish Setup"` | 按钮 |

### `crates/onboarding/src/basics_page.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 27-32 | `"One Light"`, `"Ayu Light"`, `"Gruvbox Light"`, `"One Dark"`, `"Ayu Dark"`, `"Gruvbox Dark"` | 主题名 |
| 59-69 | `"Theme"` | 章节标签 |
| 341-345 | `"Base Keymap"` | 章节标签 |
| 519-522 | `"Import Settings"`, `"Automatically pull your settings from other editors"` | 章节标签/描述 |
| 556-561 | `"Install"` | 按钮 |
| 601-624 | `"Sign In"`, `"Signing In…"`, `"Start Free Trial"` | 按钮 |
| 689-695 | `"Agent Setup"`, `"Install your favorite agents and start your first thread."` | 章节标签/描述 |

### `crates/onboarding/src/multibuffer_hint.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 150-159 | `"Edit and save files directly in the results multibuffer!"` | 提示信息 |
| 161-183 | `"Learn More"`, `"Dismiss Hint"` | 按钮/工具提示 |

---

## 16. title_bar

### `crates/title_bar/src/title_bar.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 373-383 | `"Signing in…"` | 标签 |
| 602-615 | `"Connecting to: {host}"`, `"Connected to: {host}"` | 连接状态 |
| 609-612 | `"Lost connection to {host}. Reconnecting..."` | 连接丢失 |
| 613-615 | `"Disconnected from {host}"` | 断开连接 |
| 674-684 | `"Restricted Mode"` | 按钮 |
| 722-728 | `"Disconnected"` | 按钮 |
| 739-749 | `"{} is sharing this project. Click to follow."` / `"Click to Follow"` | 工具提示 |
| 956-960 | `"Loading {}…"` / `"Creating {}…"` | 标签 |
| 989-994 | `"Worktree"`, `"Currently In Use: {}"` | 工具提示 |
| 1009-1028 | `"Create Branch"` | 按钮 |
| 1063-1068 | `"/"` | 分隔符标签 |
| 1137 | `"Disconnected"` | 工具提示 |
| 1187-1226 | `"Sign in to Wuling"`, `"Sign In"` | 按钮 |
| 1356-1360 | `"Restart to update Zed"` | 标签 |

### `crates/title_bar/src/collab.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 242-245 | `"Follow {login}"` | 工具提示 |
| 297-300 | `"{} is muted"` | 工具提示 |
| 395-403 | `"Leave Call"` | 工具提示 |
| 455-466 | `"Mute Microphone"` / `"Unmute Microphone"` | 音频工具提示 |
| 486-500 | `"Mute Audio"` / `"Unmute Audio"` | 音频工具提示 |
| 560-612 | `"Stop sharing {folder_list}..."`, `"Unshare Project"`, `"Share Project"`, `"This project may not be shared in a public channel."` | 共享工具提示 |
| 648-677 | `"Sharing Screen Failed"`, `"Sharing Screen"` / `"Share Screen"` | 屏幕共享工具提示 |

### `crates/title_bar/src/application_menu.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 164-172 | `"Open Application Menu"` | 工具提示 |

### `crates/title_bar/src/onboarding_banner.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 45-50 | `"Introducing:"` | 横幅文本 |
| 159-169 | `"Close Announcement Banner"`, `"It won't show again for this feature"` | 工具提示 |

---

## 17. auto_update_ui

### `crates/auto_update_ui/src/auto_update_ui.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 75-77 | `"Couldn't load release notes"` | 错误信息 |
| 82-84 | `"View in Browser"` | 错误操作 |
| 227-228 | `"Skills live in {GLOBAL_SKILLS_DIR_DISPLAY}/<name>/SKILL.md"`, `"Type / to manually invoke a skill"` | 公告项目符号 |
| 236-240 | `"Introducing Skills Support"`, `"Extend the agent with focused instructions and domain knowledge."`, `"Try Now"`, `"Read Documentation"` | 公告标题/描述/按钮 |
| 353-354 | `"Updated to {} {}"`, `"View Release Notes"` | 更新通知 |

---

## 18. recent_projects

### `crates/recent_projects/src/sidebar_recent_projects.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 296-300 | `"Recently opened projects will show up here"`, `"No matches"` | 空/无匹配文本 |
| 381-386 | `"Open Project in This Window"` | 工具提示 |
| 402-421 | `"Open Local Folders"`, `"Open Remote Folder"` | 页脚按钮 |

### `crates/recent_projects/src/disconnected_overlay.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 151 | `"Your connection to the remote project has been lost."` | 协作项目断开 |
| 158 | `"Unsaved changes are stored locally."` | 自动保存注释 |
| 163-166 | `"process exiting unexpectedly"`, `"not responding"` | 断开原因 |
| 168-169 | `"Your connection to {} has been lost due to the server {reason}."` | 断开消息格式 |
| 186 | `"Disconnected"` | 标题 |
| 194 | `"Close Window"` | 按钮 |
| 203 | `"Reconnect"` | 按钮 |

### `crates/recent_projects/src/remote_connection.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 144-148 | `"Toggle to Unmask Password"`, `"Toggle to Mask Password"` | 密码切换工具提示 |
| 193-197 | `"Caps lock is on."` | 提示标签 |
| 389-394 | `"Cancel"` | 退出按钮 |

---

## 19. theme_selector

### `crates/theme_selector/src/theme_selector.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 381-383 | `"Select Theme..."` | 占位符文本 |
| 541-562 | `"View Theme Docs"`, `"Install Themes"` | 页脚按钮 |

### `crates/theme_selector/src/icon_theme_selector.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 165-167 | `"Select Icon Theme..."` | 占位符文本 |
| 332-353 | `"View Icon Theme Docs"`, `"Install Icon Themes"` | 页脚按钮 |

---

## 20. notifications

> 注: `notifications` crate 主要提供框架而非界面字符串，但 `workspace/src/notifications.rs` 有预览测试字符串（已包含在工作台部分）。

---

## 21. ama10-ui

### `crates/ama10-ui/src/sign_in_modal.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 118-122 | `"Contacting server…"`, `"Requesting device code…"`, `"Waiting for approval"`, `"Signed in"`, `"Sign-in failed"` | 状态标签 |
| 232 | `"Sign in to Wuling DevOps"` | 标题 |
| 246 | `"Connecting to the Wuling DevOps server…"` | 标签 |
| 270-271 | `"Visit the URL below in your browser, then enter this code:"` | 说明 |
| 293 | `"Copied!"` / `"Copy"` | 按钮 |
| 312-334 | `"Open browser"`, `"Cancel"`, `"Or visit: {fallback_url}  ·  Code expires in {remaining}s"` | 按钮/说明 |
| 334 | `"Signed in as {username}."` | 成功标签 |
| 336 | `"Done"` | 按钮 |
| 346 | `"Close"` | 按钮 |

### `crates/ama10-ui/src/server_url_modal.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 107 | `"Wuling DevOps server URL"` | 标题 |
| 109-111 | `"Default: {}"` | 标签 |
| 135 | `"Save"` | 按钮 |
| 143 | `"Cancel"` | 按钮 |

---

## 22. 杂项 crate

### `crates/feedback/src/feedback.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 65-67 | `"Copied into clipboard"`, `"OK"` | 对话框 |
| 78-80 | `"Copied into clipboard"`, `"OK"` | 对话框 |
| 137 | `"No extensions installed."` | 无扩展文本 |

### `crates/journal/src/journal.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 201-202 | `"AM"`, `"PM"`, `"# {}:{:02} {}"` | 时间格式 |

### `crates/tab_switcher/src/tab_switcher.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 721 | `"Search all tabs…"` | 占位符文本 |
| 725 | `"No tabs"` | 无匹配文本 |
| 867 | `"Close"` | 工具提示 |

### `crates/settings_profile_selector/src/settings_profile_selector.rs`

| 行号 | 字符串 | 类型 |
|------|--------|------|
| 153 | `"Select a settings profile..."` | 占位符文本 |
| 277 | `"Disabled"` | 显示名 |

### `crates/notifications/src/` (核心框架)

无硬编码用户界面字符串（提供框架级 API，所有内容由调用者传递）。

### `crates/menu/src/`

无硬编码用户界面字符串。

### `crates/picker/src/`

无硬编码用户界面字符串（由委托提供内容）。

### `crates/which_key/src/`

无硬编码用户界面字符串（动态键绑定数据）。

### `crates/language_selector/src/`

无硬编码用户界面字符串（动态语言数据）。

### `crates/line_ending_selector/src/`

无硬编码用户界面字符串。

### `crates/encoding_selector/src/`

无硬编码用户界面字符串。

### `crates/activity_indicator/src/`

无硬编码用户界面字符串。

### `crates/breadcrumbs/src/`

无硬编码用户界面字符串。

---

## 附录：字符串模式分类

| 类别 | 示例 | i18n 建议 |
|------|------|-----------|
| 静态字符串 | `"Save"`, `"Cancel"`, `"Open File"` | 简单键值 |
| 格式化字符串 | `"Failed to save {}: {}"` | ICU MessageFormat |
| 复数形式 | `"{} file"` / `"{} files"` | ICU 复数选择 |
| 带条件的 | `"Hide"`/`"Show"`, `"Lock"`/`"Unlock"` | 上下文键 |
| 平台变体 | `"Reveal in Finder"` (macOS) vs `"Reveal in File Explorer"` (Windows) | 平台上下文 |
| 菜单栏 | `"File" > "Save As…"` | 嵌套键结构 |
| 设置项 | `"Font Size — Font size for editor text."` | 标题/描述对 |
| 工具提示 | `"Click to Close"`, `"Shift-click to Suppress"` | 简短描述 |
| 错误信息 | `"Failed to load your settings..."` | 模板 |
| 空状态 | `"No extensions that match your search."` | 多条件变体 |
