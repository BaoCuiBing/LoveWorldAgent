// UiChrome 实现：查找、缩放、截图、清数据/导入、保持焦点。与「更多」菜单保留的 5 项一一对应；打印/历史/下载/设置/设备工具栏/在外部打开已随 AI 场景裁剪移除。
#pragma once // 防止头文件重复包含
#include <string> // 标准字符串
#include "include/cef_browser.h" // CefBrowser
#include "include/cef_values.h" // CEF 值类型
class AppWindow; // 前置声明
class CdpClient; // 前置声明
class UiChrome { // 外壳动作集
public:
    explicit UiChrome(AppWindow* window); // 绑定窗口
    void RunPipeCommand(int id, const std::string& cmd, CefRefPtr<CefDictionaryValue> args); // 管道侧入口：ui_* 指令与菜单动作共用实现，id!=0 时回响应
    void ApplyInitialSettings(int zoom_percent, bool keep_focus); // 启动时从 --settings 继承的缩放与保持焦点开关
    void ShowFind(); // 打开查找条
    void HideFind(); // 关闭查找条
    void FindNext(bool forward); // 查找下一处/上一处
    int zoom_percent() const { return zoom_percent_; } // 当前缩放
    void SetZoomPercent(int percent); // 设置缩放
    void ZoomStep(int delta); // 缩放步进
    bool keep_focus() const { return keep_focus_; } // 保持焦点开关
    void ToggleKeepFocus(); // 切换保持焦点
    void SetKeepFocus(bool value); // 直接设置保持焦点
    void ClearBrowsingData(int id); // 数据清理（Cookie / 缓存 / localStorage）
    void ImportBrowsingData(int id, CefRefPtr<CefDictionaryValue> payload); // 数据导入
private:
    void ReportUi(const std::string& item, CefRefPtr<CefDictionaryValue> extra); // ui_* 指令回包
    void ApplyZoom(); // 应用缩放到活动标签
    void ClearStorageViaJs(); // 经 JS 清 localStorage/sessionStorage
    AppWindow* window_; // 宿主窗口
    int zoom_percent_ = 100; // 当前缩放百分比
    bool find_started_ = false; // 首次 Find 从头匹配，后续走 findNext
    bool keep_focus_ = false; // 保持焦点开关
};
