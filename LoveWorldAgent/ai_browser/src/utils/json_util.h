#pragma once // 防止头文件重复包含
#include <windows.h> // Windows 基础类型
#include <string> // 标准字符串
#include "include/cef_values.h" // CEF 值类型
// 协议 JSON 与编码工具：解析/序列化直接复用 CEF 的 CefParseJSON/CefWriteJSON，不手写字符串扫描（旧实现只支持一层扁平对象且反转义有错）
std::wstring Utf8ToWide(const std::string& text); // UTF-8 → UTF-16
std::string WideToUtf8(const std::wstring& text); // UTF-16 → UTF-8
CefRefPtr<CefDictionaryValue> ParseJsonObject(const std::string& utf8); // 解析一行 UTF-8 JSON，非对象或失败返回 nullptr
std::string WriteJson(CefRefPtr<CefValue> value); // 序列化任意 CefValue 为 UTF-8 JSON 文本
CefRefPtr<CefValue> DictValue(CefRefPtr<CefDictionaryValue> dict); // 字典套一层 CefValue
CefRefPtr<CefValue> ListValue(CefRefPtr<CefListValue> list); // 列表套一层 CefValue
CefRefPtr<CefValue> StringValue(const std::string& utf8); // 字符串套一层 CefValue
std::string WriteJsonDict(CefRefPtr<CefDictionaryValue> dict); // 字典 → JSON 文本
std::string WriteJsonList(CefRefPtr<CefListValue> list); // 列表 → JSON 文本
bool ArgGetString(CefRefPtr<CefDictionaryValue> args, const std::string& key, std::string& out); // 读字符串参数，缺失或类型不符返回 false（out 保持原值）
bool ArgGetInt(CefRefPtr<CefDictionaryValue> args, const std::string& key, int& out); // 读整型参数
bool ArgGetDouble(CefRefPtr<CefDictionaryValue> args, const std::string& key, double& out); // 读浮点参数
bool ArgGetBool(CefRefPtr<CefDictionaryValue> args, const std::string& key, bool& out); // 读布尔参数
CefRefPtr<CefListValue> ArgGetList(CefRefPtr<CefDictionaryValue> args, const std::string& key); // 读列表参数
CefRefPtr<CefDictionaryValue> ArgGetDict(CefRefPtr<CefDictionaryValue> args, const std::string& key); // 读字典参数
CefRefPtr<CefDictionaryValue> MakeDict(); // 新建空字典
CefRefPtr<CefListValue> MakeList(); // 新建空列表
void ListAddString(CefRefPtr<CefListValue> list, const std::string& value); // 列表尾部追加字符串
void ListAddDict(CefRefPtr<CefListValue> list, CefRefPtr<CefDictionaryValue> value); // 列表尾部追加字典
std::string StrOf(CefRefPtr<CefDictionaryValue> dict, const std::string& key); // 取字符串值（缺失回空串）
int IntOf(CefRefPtr<CefDictionaryValue> dict, const std::string& key); // 取整型值（缺失回 0）
bool BoolOf(CefRefPtr<CefDictionaryValue> dict, const std::string& key); // 取布尔值（缺失回 false）
std::string Utf8Slice(const std::string& text, size_t start, size_t max_bytes, size_t& next); // 按 UTF-8 字符边界从 start 取不超过 max_bytes 的片段，next 返回下一片段起始下标
