// ToolRegistry 实现：注册全部 browser_* 工具并按名分发
#include "tool_registry.h" // 自身声明
#include "browser_click.h" // 单击
#include "browser_console_messages.h" // 控制台消息
#include "browser_evaluate.h" // 执行 JS
#include "browser_get_attribute.h" // 读属性
#include "browser_handle_dialog.h" // 应答弹窗
#include "browser_hover.h" // 悬停
#include "browser_navigate.h" // 导航
#include "browser_navigate_back.h" // 后退/前进
#include "browser_network_requests.h" // 网络请求
#include "browser_press_key.h" // 按键
#include "browser_read_text.h" // 读文本
#include "browser_scroll.h" // 滚动
#include "browser_select_option.h" // 下拉选择
#include "browser_snapshot.h" // 可访问性快照
#include "browser_tabs.h" // 标签管理
#include "browser_take_screenshot.h" // 截图
#include "browser_type.h" // 输入
#include "browser_wait_for.h" // 等待
#include "browser_waiting_for_user_interaction.h" // 交还用户
#include "utils/logging.h" // 文件日志
ToolRegistry::ToolRegistry() { tools_.push_back(std::make_unique<BrowserNavigateTool>()); tools_.push_back(std::make_unique<BrowserNavigateBackTool>()); tools_.push_back(std::make_unique<BrowserTabsTool>()); tools_.push_back(std::make_unique<BrowserSnapshotTool>()); tools_.push_back(std::make_unique<BrowserReadTextTool>()); tools_.push_back(std::make_unique<BrowserTakeScreenshotTool>()); tools_.push_back(std::make_unique<BrowserClickTool>()); tools_.push_back(std::make_unique<BrowserHoverTool>()); tools_.push_back(std::make_unique<BrowserTypeTool>()); tools_.push_back(std::make_unique<BrowserSelectOptionTool>()); tools_.push_back(std::make_unique<BrowserPressKeyTool>()); tools_.push_back(std::make_unique<BrowserGetAttributeTool>()); tools_.push_back(std::make_unique<BrowserScrollTool>()); tools_.push_back(std::make_unique<BrowserWaitForTool>()); tools_.push_back(std::make_unique<BrowserWaitingForUserTool>()); tools_.push_back(std::make_unique<BrowserEvaluateTool>()); tools_.push_back(std::make_unique<BrowserHandleDialogTool>()); tools_.push_back(std::make_unique<BrowserConsoleMessagesTool>()); tools_.push_back(std::make_unique<BrowserNetworkRequestsTool>()); } // 注册 19 个 browser_* 工具
bool ToolRegistry::Dispatch(const ToolRequest& req) const { LOG_DEBUG("tool", QStringLiteral("分发指令 %1（id=%2）").arg(QString::fromStdString(req.cmd)).arg(req.id)); for (const std::unique_ptr<IBrowserTool>& entry : tools_) { if (req.cmd == entry->name()) { entry->Run(req); return true; } } LOG_WARN("tool", QStringLiteral("未知指令 %1（id=%2），已忽略").arg(QString::fromStdString(req.cmd)).arg(req.id)); return false; } // 按指令名分发，命中即执行；未命中记 WARN 便于排查协议漂移
std::vector<std::string> ToolRegistry::names() const { std::vector<std::string> out; for (const std::unique_ptr<IBrowserTool>& tool : tools_) out.emplace_back(tool->name()); return out; } // 收集全部工具名
