#pragma once // 防止头文件重复包含
#include <cstdint> // uint32_t
#include <string> // 标准字符串
// 页面脚本构造工具：ref 安全校验、元素定位模板、JS 字面量转义、按键表与共享脚本；各 browser_* 工具都靠这里生成注入 JS，避免重复拼字符串
namespace js {
extern const char* kSnapshot; // 可访问性树快照：清掉旧 ref 后重新编号，交互元素写 data-lwa-ref + __lwaRefMap，优先保留交互元素
extern const char* kReadText; // 页面可见文本（截断到 2 万字）
std::string SafeRef(const std::string& ref); // ref 只允许安全字符，避免拼进 JS 字符串时被注入
std::string RefSelector(const std::string& ref); // 返回 [data-lwa-ref="e12"] 选择器
std::string ResolveRefExpr(const std::string& ref); // 生成双层 ref 定位表达式（__lwaRefMap -> data-lwa-ref -> 唯一指纹，失效抛错）
std::string Element(const std::string& ref, const std::string& tail); // 定位元素后执行 tail；元素不存在即抛错，由 Runtime.evaluate 的 exceptionDetails 回传
std::string Literal(const std::string& utf8); // 把任意字符串安全嵌入 JS 源码（双引号字面量）；不能用 CefWriteJSON：它对纯字符串根返回空串
int MouseButtonCode(const std::string& raw); // left / right / middle 归一，并给出 MouseEvent.button 的数值
struct KeyInfo { std::string name; std::string code; int vk = 0; char16_t ch = 0; bool printable = false; uint32_t modifiers = 0; }; // 键信息：key 名、code、虚拟键码、字符与修饰键掩码
bool QueryKey(const std::string& key, KeyInfo& out); // 命名键（含 F1-F12、修饰键组合 Ctrl+a 等）与单字符/数字/标点换算
}
