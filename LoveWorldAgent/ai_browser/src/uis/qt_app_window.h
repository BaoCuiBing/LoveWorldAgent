#pragma once // 防止头文件重复包含
#include <QMainWindow> // Qt 主窗口
#include "uis/app_window.h" // 外壳纯虚接口
class BcbLineEdit; // Bcb 地址栏
class BcbButton; // Bcb 按钮
class BcbOriginalTabWidget; // Bcb 标签控件
class QMenu; // 「更多」菜单
class CursorOverlay; // 智能体模拟鼠标覆盖层
class QtAppWindow : public QMainWindow, public AppWindow { // Qt 版外壳窗口
    Q_OBJECT // Qt 元对象宏
public:
    static QtAppWindow* Create(); // 创建（或复用）Qt 版面板窗口单例；失败返回 nullptr
    HWND hwnd() const override; // 原生窗口句柄
    HWND HostHwnd() const override; // CEF 承载区原生句柄
    TabManager* Tabs() override { return &tabs_; } // 标签管理器
    AgentApi* Api() override { return &api_; } // 指令入口
    UiChrome* Chrome() override { return &chrome_; } // 外壳动作集
    bool IsVisible() const override; // 是否可见
    RECT BrowserArea() const override; // CEF 内容区矩形
    CefRefPtr<CefBrowser> ActiveBrowser() override; // 活动浏览器
    CefRefPtr<CdpClient> ActiveCdp() override; // 活动 CDP
    std::wstring ActiveUrl() override; // 活动地址
    void SetFindVisible(bool visible) override; // 查找条显隐
    void SetFindText(const std::wstring& text) override; // 写入查找词
    bool find_visible() const override { return findVisible_; } // 查找条状态
    void BeginBrowserClose() override; // 抑制宿主 WM_CLOSE 误退出（带看门狗自动复位）
    void EndBrowserClose() override { browserClosing_ = false; } // 解除抑制
    std::wstring FindInputText() const override; // 读取查找输入
    void SyncZoomUi(int percent) override; // 同步缩放值
    void Defer(std::function<void()> task) override; // 投递 UI 线程
    void OnBrowserCreated(CefRefPtr<CefBrowser> browser, CefRefPtr<AppHandler> handler) override; // 浏览器创建
    void OnBrowserClosed(CefRefPtr<CefBrowser> browser) override; // 浏览器关闭
    void OnTitle(CefRefPtr<CefBrowser> browser, const std::wstring& title) override; // 标题
    void OnAddress(CefRefPtr<CefBrowser> browser, const std::wstring& address) override; // 地址
    void OnLoadingState(CefRefPtr<CefBrowser> browser, bool loading, bool can_go_back, bool can_go_forward) override; // 加载状态
    void OnConsole(CefRefPtr<CefBrowser> browser, cef_log_severity_t level, const std::string& text, const std::string& source, int line) override; // 控制台
    void OpenPopup(const std::wstring& url) override; // 弹窗转新标签
    void OnTabsChanged() override; // 标签集同步 UI 与上报
    void OnAllTabsClosed() override; // 全部标签关闭
    void ScheduleTabRestore() override; // 经消息循环补建标签
    void ShowSelf(bool visible) override; // 显隐窗口
    void FocusSelf() override; // 聚焦
    void CloseSelf() override; // 仅隐藏保活
    void HideSelf() override; // 隐藏
    void QuitSelf() override; // 真正退出
    void SetRect(const RECT& rect) override; // 设置矩形
    void ToggleFullscreen() override; // 切全屏
    void ShowAgentCursor(double x, double y, double vw, double vh, bool click) override; // 页面视口坐标 → 覆盖层坐标并显示模拟鼠标
    void HideAgentCursor() override; // 隐藏模拟鼠标
protected:
    bool nativeEvent(const QByteArray& eventType, void* message, qintptr* result) override; // 接收管道线程 WM_APP_* 消息
    void resizeEvent(QResizeEvent* event) override; // 尺寸变化同步 CEF 子窗口与光标覆盖层
    void moveEvent(QMoveEvent* event) override; // 窗口移动时同步光标覆盖层
    void closeEvent(QCloseEvent* event) override; // 关闭仅隐藏保活
private:
    QtAppWindow(); // 私有构造（经 Create）
    void BuildUi(); // 构建工具栏/标签/内容区/查找条
    void NavigateFromAddress(); // 地址栏回车导航
    void RunToolCommand(const std::string& cmd); // 菜单命令执行
    void BuildMoreMenu(); // 构建「更多」菜单
    void OnTabChanged(int index); // 切换标签
    void OnTabCloseRequested(int index); // 关闭标签
    void OnFindNext(bool forward); // 查找下一处
    void FetchFavicon(CefRefPtr<CefBrowser> browser); // 加载完成后取页面 favicon：CDP 读 link[rel~=icon]，空则回退 /favicon.ico
    void DownloadFavicon(CefRefPtr<CefBrowser> browser, const std::string& url); // 经 CefURLRequest 下载图标并解码
    void SetTabIcon(BrowserTab* tab, const QIcon& icon); // 写回标签图标并刷新标签栏对应下标
    QIcon DefaultIcon() const; // resources/dragon.ico 生成的默认图标（加载中/失败兜底）
    void UpdateCursorBounds(); // 光标覆盖层对齐 CEF 承载区全局矩形
    QWidget* cefHost_ = nullptr; // CEF 承载区（其原生窗口作为浏览器子窗口的父级，避免被 Qt 原生子窗口遮盖）
    BcbLineEdit* address_ = nullptr; // 地址栏
    BcbButton* btnBack_ = nullptr; // 后退
    BcbButton* btnForward_ = nullptr; // 前进
    BcbButton* btnReload_ = nullptr; // 刷新
    BcbButton* btnNewTab_ = nullptr; // 新建标签
    BcbButton* btnMore_ = nullptr; // 更多菜单
    BcbOriginalTabWidget* tabsWidget_ = nullptr; // 标签栏
    QWidget* findBar_ = nullptr; // 查找条容器
    BcbLineEdit* findEdit_ = nullptr; // 查找输入
    QMenu* moreMenu_ = nullptr; // 「更多」菜单
    TabManager tabs_{this}; // 标签管理
    AgentApi api_{this}; // 指令入口
    UiChrome chrome_{this}; // 外壳动作
    CursorOverlay* cursor_ = nullptr; // 智能体模拟鼠标覆盖层（独立顶层透明窗口）
    bool browserClosing_ = false; // 标签关闭抑制标记
    bool quitting_ = false; // 退出标记
    bool fullscreen_ = false; // 全屏标记
    bool findVisible_ = false; // 查找条可见
    std::wstring findText_; // 当前查找词
    RECT savedRect_ = {0, 0, 0, 0}; // 全屏前矩形
    DWORD savedStyle_ = 0; // 全屏前样式（预留）
    int zoomPercent_ = 100; // 当前缩放
};
