#include "uis/qt_app_window.h"
#include <windows.h> // Win32 窗口句柄与消息
#include <shellapi.h> // SHGetPropertyStoreForWindow
#include <shobjidl.h> // IPropertyStore
#include <propkey.h> // PKEY_AppUserModel_*
#include <QApplication>
#include <QCoreApplication>
#include <QDir>
#include <QHBoxLayout>
#include <QIcon> // 标签页图标
#include <QImage>
#include <QMenu>
#include <QMessageBox>
#include <QMoveEvent> // 窗口移动时同步光标覆盖层
#include <QResizeEvent>
#include <QSignalBlocker> // 重建标签栏时临时屏蔽 currentChanged，避免与 Select() 互相回调
#include <QTimer>
#include <QVBoxLayout>
#include <vector> // 图标下载缓冲
#include "include/cef_urlrequest.h" // CefURLRequest 下载 favicon
#include "components/bcb_button.h"
#include "components/bcb_line_edit.h"
#include "components/bcb_original_tab_widget.h"
#include "uis/qt_theme.h"
#include "uis/cursor_overlay.h" // 智能体模拟鼠标覆盖层
#include "server/client_app.h"
#include "server/pipe_client.h"
#include "server/app_handler.h"
#include "server/cdp_client.h"
#include "uis/tab_manager.h"
#include "uis/ui_chrome.h"
#include "utils/config.h" // 资源路径解析（默认图标）
#include "utils/json_util.h"
#include "utils/logging.h" // 文件日志
namespace {
void ApplyTaskbarGrouping(HWND hwnd) { // 设置窗口级 AppUserModelID，与主应用在 Windows 任务栏同一分组重叠展示
    if (hwnd == nullptr || !::IsWindow(hwnd)) return;
    const HRESULT co_hr = ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    const bool co_init = (co_hr == S_OK || co_hr == S_FALSE);
    IPropertyStore* store = nullptr;
    const HRESULT hr = ::SHGetPropertyStoreForWindow(hwnd, IID_PPV_ARGS(&store));
    if (SUCCEEDED(hr) && store != nullptr) {
        std::wstring icon_res;
        wchar_t exe_path[MAX_PATH] = {0};
        if (::GetModuleFileNameW(nullptr, exe_path, MAX_PATH) > 0) {
            icon_res = std::wstring(exe_path) + L",0";
        }
        auto set_str = [store](const PROPERTYKEY& key, const wchar_t* val) {
            PROPVARIANT pv;
            ::PropVariantInit(&pv);
            pv.vt = VT_LPWSTR;
            pv.pwszVal = const_cast<LPWSTR>(val);
            store->SetValue(key, pv);
        };
        set_str(PKEY_AppUserModel_ID, L"com.loveworld.agent");
        set_str(PKEY_AppUserModel_RelaunchDisplayNameResource, L"LoveWorldAgent");
        if (!icon_res.empty()) {
            set_str(PKEY_AppUserModel_RelaunchIconResource, icon_res.c_str());
        }
        store->Commit();
        store->Release();
    }
    if (co_init) ::CoUninitialize();
}
}
QtAppWindow::QtAppWindow() : QMainWindow(nullptr) {} // 无父级：独立顶层窗口
QtAppWindow* QtAppWindow::Create() { static QtAppWindow* s_instance = nullptr; if (s_instance) return s_instance; QtAppWindow* self = new QtAppWindow(); try { self->BuildUi(); self->setWindowTitle(QString::fromWCharArray(L"内置浏览器")); self->setWindowIcon(QApplication::windowIcon()); self->setMinimumSize(640, 480); if (g_options.has_rect) { self->setGeometry(g_options.rect.left, g_options.rect.top, g_options.rect.right - g_options.rect.left, g_options.rect.bottom - g_options.rect.top); } else { self->resize(1200, 800); } self->show(); QtTheme::Apply(); ApplyTaskbarGrouping(self->hwnd()); self->cefHost_->winId(); self->cursor_ = new CursorOverlay(); self->UpdateCursorBounds(); self->chrome_.ApplyInitialSettings(g_options.zoom_percent, g_options.keep_focus); self->chrome_.SetZoomPercent(g_options.zoom_percent); PipeClient::SetTarget(self->hwnd()); s_instance = self; LOG_INFO("window", QStringLiteral("主窗口就绪：句柄=%1，承载区=%2").arg(static_cast<qulonglong>(reinterpret_cast<quintptr>(self->hwnd()))).arg(static_cast<qulonglong>(reinterpret_cast<quintptr>(self->HostHwnd())))); return self; } catch (...) { LOG_ERROR("window", QStringLiteral("主窗口创建异常，已回收")); delete self; return nullptr; } } // 单例创建：独立顶层窗口 + 共享 AppUserModelID（无 GWLP_HWNDPARENT 耦合）；建窗口→标题/图标→几何→显示→QSS→任务栏分组属性→CEF 承载区→初始设置→管道目标
void QtAppWindow::BuildUi() { auto* central = new QWidget(this); auto* layout = new QVBoxLayout(central); layout->setContentsMargins(0, 0, 0, 0); layout->setSpacing(0); auto* nav = new QWidget(central); auto* navLayout = new QHBoxLayout(nav); navLayout->setContentsMargins(6, 4, 6, 4); navLayout->setSpacing(4); btnBack_ = new BcbButton(QString::fromWCharArray(L"后退"), nav); btnForward_ = new BcbButton(QString::fromWCharArray(L"前进"), nav); btnReload_ = new BcbButton(QString::fromWCharArray(L"刷新"), nav); btnNewTab_ = new BcbButton(QString::fromWCharArray(L"新建标签"), nav); address_ = new BcbLineEdit(nav); address_->setPlaceholderText(QString::fromWCharArray(L"输入网址后回车")); btnMore_ = new BcbButton(QString::fromWCharArray(L"更多"), nav); navLayout->addWidget(btnBack_); navLayout->addWidget(btnForward_); navLayout->addWidget(btnReload_); navLayout->addWidget(btnNewTab_); navLayout->addWidget(address_, 1); navLayout->addWidget(btnMore_); tabsWidget_ = new BcbOriginalTabWidget(central); tabsWidget_->setTabsClosable(true); tabsWidget_->setMovable(false); tabsWidget_->setElideMode(Qt::ElideRight); tabsWidget_->setMaxTabTextLength(40); cefHost_ = new QWidget(central); cefHost_->setMinimumSize(100, 100); cefHost_->setStyleSheet("background:#fff;"); findBar_ = new QWidget(central); findBar_->setVisible(false); auto* findLayout = new QHBoxLayout(findBar_); findLayout->setContentsMargins(6, 4, 6, 4); findLayout->setSpacing(4); findEdit_ = new BcbLineEdit(findBar_); findEdit_->setPlaceholderText(QString::fromWCharArray(L"查找")); auto* btnPrev = new BcbButton(QString::fromWCharArray(L"上一处"), findBar_); auto* btnNext = new BcbButton(QString::fromWCharArray(L"下一处"), findBar_); findLayout->addWidget(findEdit_, 1); findLayout->addWidget(btnPrev); findLayout->addWidget(btnNext); layout->addWidget(nav); layout->addWidget(tabsWidget_); layout->addWidget(cefHost_, 1); layout->addWidget(findBar_); setCentralWidget(central); BuildMoreMenu(); connect(btnBack_, &BcbButton::clicked, [this]() { CefRefPtr<CefBrowser> b = ActiveBrowser(); if (b) b->GoBack(); }); connect(btnForward_, &BcbButton::clicked, [this]() { CefRefPtr<CefBrowser> b = ActiveBrowser(); if (b) b->GoForward(); }); connect(btnReload_, &BcbButton::clicked, [this]() { CefRefPtr<CefBrowser> b = ActiveBrowser(); if (b) b->Reload(); }); connect(btnNewTab_, &BcbButton::clicked, [this]() { tabs_.AddTab(g_options.start_url, -1); }); connect(btnMore_, &BcbButton::clicked, [this]() { BuildMoreMenu(); moreMenu_->popup(btnMore_->mapToGlobal(QPoint(0, btnMore_->height()))); }); connect(address_, &BcbLineEdit::returnPressed, this, &QtAppWindow::NavigateFromAddress); connect(tabsWidget_, &BcbOriginalTabWidget::currentChanged, this, &QtAppWindow::OnTabChanged); connect(tabsWidget_, &BcbOriginalTabWidget::tabCloseRequested, this, &QtAppWindow::OnTabCloseRequested); connect(findEdit_, &BcbLineEdit::returnPressed, [this]() { OnFindNext(true); }); connect(btnPrev, &BcbButton::clicked, [this]() { OnFindNext(false); }); connect(btnNext, &BcbButton::clicked, [this]() { OnFindNext(true); }); } // 构建工具栏（BcbButton 后退/前进/刷新/新建/地址/更多）→ BcbOriginalTabWidget 标签栏 → CEF 承载区（白底）→ 隐藏查找条；全部信号接线
void QtAppWindow::BuildMoreMenu() { if (!moreMenu_) moreMenu_ = new QMenu(this); moreMenu_->clear(); auto add = [this](const QString& label, const std::string& cmd) { QAction* a = moreMenu_->addAction(label); connect(a, &QAction::triggered, [this, cmd]() { RunToolCommand(cmd); }); }; add(QString::fromWCharArray(L"查找"), "ui_find"); add(QString::fromWCharArray(L"截图"), "ui_screenshot"); add(QString::fromWCharArray(L"清除浏览数据"), "ui_clear_data"); add(QString::fromWCharArray(L"导入数据"), "ui_import_data"); add(QString::fromWCharArray(L"保持焦点"), "ui_keep_focus"); } // 「更多」菜单：5 项对 AI 场景有实际价值的外壳动作；每次弹出前重建以刷新状态
void QtAppWindow::RunToolCommand(const std::string& cmd) { CefRefPtr<CefDictionaryValue> args = CefDictionaryValue::Create(); chrome_.RunPipeCommand(0, cmd, args); } // 菜单命令经 chrome 复用 ui_* 实现（id=0 不回包）
void QtAppWindow::NavigateFromAddress() { const std::wstring url = address_->text().toStdWString(); if (url.empty()) return; CefRefPtr<CefBrowser> b = ActiveBrowser(); if (b) b->GetMainFrame()->LoadURL(WideToUtf8(url)); } // 地址栏回车：活动标签主框架加载
void QtAppWindow::OnFindNext(bool forward) { findText_ = findEdit_->text().toStdWString(); if (findText_.empty()) return; SetFindText(findText_); chrome_.FindNext(forward); } // 查找：记录词并走 chrome 的 Find 逻辑
void QtAppWindow::OnTabChanged(int index) { if (index >= 0 && index < tabs_.count()) tabs_.Select(index); } // 标签切换同步到管理器
void QtAppWindow::OnTabCloseRequested(int index) { if (index >= 0 && index < tabs_.count()) tabs_.Close(index); } // 标签关闭
HWND QtAppWindow::hwnd() const { return reinterpret_cast<HWND>(winId()); } // Qt 窗口原生句柄
HWND QtAppWindow::HostHwnd() const { return reinterpret_cast<HWND>(cefHost_->winId()); } // 承载区原生句柄：浏览器子窗口的父级
bool QtAppWindow::IsVisible() const { return isVisible() && !isMinimized(); } // 可见性（最小化视为不可见，走 JS 兜底）
RECT QtAppWindow::BrowserArea() const { const qreal dpr = cefHost_->devicePixelRatioF(); QSize s = cefHost_->size(); RECT r; r.left = 0; r.top = 0; r.right = static_cast<LONG>(s.width() * dpr); r.bottom = static_cast<LONG>(s.height() * dpr); return r; } // 承载区尺寸换算为物理像素（SetAsChild/MoveWindow 只认设备像素，直接用 Qt 逻辑尺寸在 125% 缩放下会只铺满 80%，右下露白底）
CefRefPtr<CefBrowser> QtAppWindow::ActiveBrowser() { BrowserTab* t = tabs_.Active(); return t ? t->browser : nullptr; } // 活动浏览器（异步建页中可能为空）
CefRefPtr<CdpClient> QtAppWindow::ActiveCdp() { BrowserTab* t = tabs_.Active(); return t ? t->cdp : nullptr; } // 活动标签 CDP
std::wstring QtAppWindow::ActiveUrl() { BrowserTab* t = tabs_.Active(); return t ? t->url : std::wstring(); } // 活动标签地址
void QtAppWindow::SetFindVisible(bool visible) { findVisible_ = visible; findBar_->setVisible(visible); if (visible) findEdit_->setFocus(); } // 查找条显隐并聚焦输入
void QtAppWindow::SetFindText(const std::wstring& text) { findText_ = text; findEdit_->setText(QString::fromStdWString(text)); } // 写入查找词（管道侧与 UI 同步）
std::wstring QtAppWindow::FindInputText() const { return findEdit_->text().toStdWString(); } // 读取查找输入框
void QtAppWindow::SyncZoomUi(int percent) { zoomPercent_ = percent; } // 同步缩放值（地址栏侧无展示，仅记录）
void QtAppWindow::Defer(std::function<void()> task) { QTimer::singleShot(0, this, [task]() { task(); }); } // 0ms 定时器回 UI 线程执行
void QtAppWindow::OnBrowserCreated(CefRefPtr<CefBrowser> browser, CefRefPtr<AppHandler> handler) { tabs_.OnBrowserCreated(browser, handler); tabs_.LayoutActive(); OnTabsChanged(); api_.OnTabReady(); } // 异步建页完成：登记标签、布局、同步 UI 与上报，并冲入初始化排队指令
void QtAppWindow::OnBrowserClosed(CefRefPtr<CefBrowser> browser) { tabs_.OnBrowserClosed(browser); tabs_.LayoutActive(); OnTabsChanged(); } // 浏览器销毁：移除标签并刷新
void QtAppWindow::OnTitle(CefRefPtr<CefBrowser> browser, const std::wstring& title) { BrowserTab* tab = tabs_.Find(browser); if (!tab) return; tab->title = title; if (tab == tabs_.Active()) { const int idx = tabs_.active(); if (idx >= 0 && idx < tabsWidget_->count()) tabsWidget_->setTabText(idx, QString::fromStdWString(title.empty() ? L"新标签" : title)); CefRefPtr<CefDictionaryValue> payload = CefDictionaryValue::Create(); payload->SetString("text", WideToUtf8(title)); PipeClient::PostEvent("title", payload); } } // 标题：写回标签缓存；活动标签同步 tab 文本并上报 title 事件
void QtAppWindow::OnAddress(CefRefPtr<CefBrowser> browser, const std::wstring& address) {
    BrowserTab* tab = tabs_.Find(browser);
    if (tab) tab->url = address;
    if (tab == tabs_.Active()) {
        if (!address_->hasFocus()) address_->setText(QString::fromStdWString(address));
        CefRefPtr<CefDictionaryValue> payload = MakeDict();
        payload->SetString("url", WideToUtf8(address));
        PipeClient::PostEvent("address", payload);
        OnTabsChanged();
    }
} // 地址变化：写回标签缓存、刷新地址栏并上报 address 与 tabs 事件
void QtAppWindow::OnLoadingState(CefRefPtr<CefBrowser> browser, bool loading, bool can_go_back, bool can_go_forward) {
    BrowserTab* tab = tabs_.Find(browser);
    if (tab) {
        tab->loading = loading;
        tab->can_back = can_go_back;
        tab->can_forward = can_go_forward;
    }
    if (tab == tabs_.Active()) {
        btnBack_->setEnabled(can_go_back);
        btnForward_->setEnabled(can_go_forward);
        CefRefPtr<CefDictionaryValue> payload = MakeDict();
        payload->SetBool("value", loading);
        payload->SetBool("canGoBack", can_go_back);
        payload->SetBool("canGoForward", can_go_forward);
        PipeClient::PostEvent("loading", payload);
    }
    if (!loading) FetchFavicon(browser);
} // 加载状态：更新标签状态、驱动按钮、上报 loading 事件并在加载完成后抓取 favicon
void QtAppWindow::OnConsole(CefRefPtr<CefBrowser> browser, cef_log_severity_t level, const std::string& text, const std::string& source, int line) { BrowserTab* tab = tabs_.Find(browser); if (tab && tab->cdp) { ConsoleEntry e; e.level = ConsoleLevelName(level); e.text = text; e.source = source; e.line = line; tab->cdp->AddConsole(e); } } // 控制台消息入对应标签的环形缓冲
void QtAppWindow::OpenPopup(const std::wstring& url) { tabs_.AddTab(url, -1); } // 弹窗统一转末尾新标签
void QtAppWindow::OnTabsChanged() {
    const QSignalBlocker blocker(tabsWidget_);
    while (tabsWidget_->count() > 0) { QWidget* w = tabsWidget_->widget(0); tabsWidget_->removeTab(0); if (w) w->deleteLater(); }
    CefRefPtr<CefListValue> list = MakeList();
    for (int i = 0; i < tabs_.count(); ++i) {
        BrowserTab* t = tabs_.At(i);
        QString title = QString::fromWCharArray(L"新标签");
        QIcon icon;
        if (t) {
            if (!t->title.empty()) title = QString::fromStdWString(t->title);
            else if (!t->url.empty()) title = QString::fromStdWString(t->url);
            icon = t->icon;
        }
        int idx = tabsWidget_->addTab(new QWidget(this), title);
        if (!icon.isNull()) tabsWidget_->setTabIcon(idx, icon);
        CefRefPtr<CefDictionaryValue> item = MakeDict();
        item->SetInt("index", i);
        item->SetString("title", t ? WideToUtf8(t->title) : std::string());
        item->SetString("url", t ? WideToUtf8(t->url) : std::string());
        item->SetBool("loading", t ? t->loading : false);
        item->SetBool("canGoBack", t ? t->can_back : false);
        item->SetBool("canGoForward", t ? t->can_forward : false);
        item->SetBool("active", i == tabs_.active());
        ListAddDict(list, item);
    }
    tabsWidget_->setCurrentIndex(tabs_.active());
    if (BrowserTab* active = tabs_.Active()) {
        if (!address_->hasFocus()) address_->setText(QString::fromStdWString(active->url));
        btnBack_->setEnabled(active->can_back);
        btnForward_->setEnabled(active->can_forward);
    }
    CefRefPtr<CefDictionaryValue> payload = MakeDict();
    payload->SetInt("n", tabs_.count());
    payload->SetInt("active", tabs_.active());
    payload->SetList("tabs", list);
    PipeClient::PostEvent("tabs", payload);
} // 标签栏全量重建：同步地址栏与前后退按钮，并上报含完整 tabs 数组的 tabs 事件
void QtAppWindow::OnAllTabsClosed() {
    if (quitting_) {
        if (cursor_) {
            cursor_->hide();
            cursor_->close();
            delete cursor_;
            cursor_ = nullptr;
        }
        close();
        QCoreApplication::quit();
    } else {
        HideSelf();
    }
} // 全部标签关闭：退出流程清理覆盖层、关窗并结束事件循环，否则仅隐藏保活
void QtAppWindow::ScheduleTabRestore() { tabs_.EnsureOneTab(); tabs_.LayoutActive(); } // 经消息循环补建起始页标签
void QtAppWindow::ShowSelf(bool visible) { setVisible(visible); if (visible && isMinimized()) showNormal(); if (visible) UpdateCursorBounds(); else HideAgentCursor(); } // 显隐窗口；显示时还原最小化并对齐光标覆盖层，隐藏时一并收起光标
void QtAppWindow::FocusSelf() { if (isMinimized()) showNormal(); activateWindow(); raise(); } // 还原最小化并激活置顶
void QtAppWindow::CloseSelf() { HideAgentCursor(); hide(); } // 关闭仅隐藏（保活，window_quit 才退出）
void QtAppWindow::HideSelf() { HideAgentCursor(); hide(); } // 隐藏
void QtAppWindow::QuitSelf() {
    quitting_ = true;
    LOG_INFO("window", QStringLiteral("收到退出指令，关闭全部标签"));
    if (cursor_) {
        cursor_->hide();
        cursor_->close();
        delete cursor_;
        cursor_ = nullptr;
    }
    tabs_.CloseAll();
    QTimer::singleShot(1500, this, [this]() {
        if (quitting_) {
            close();
            QCoreApplication::quit();
        }
    });
} // 真正退出：清理覆盖层后关全部标签，并挂兜底定时器确保事件循环退出
void QtAppWindow::SetRect(const RECT& rect) { setGeometry(rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top); } // 设置窗口矩形
void QtAppWindow::ToggleFullscreen() { if (fullscreen_) { showNormal(); setGeometry(savedRect_.left, savedRect_.top, savedRect_.right - savedRect_.left, savedRect_.bottom - savedRect_.top); fullscreen_ = false; } else { savedRect_ = {static_cast<LONG>(x()), static_cast<LONG>(y()), static_cast<LONG>(x() + width()), static_cast<LONG>(y() + height())}; showFullScreen(); fullscreen_ = true; } } // 全屏切换：进入前保存矩形，退出还原
void QtAppWindow::BeginBrowserClose() { browserClosing_ = true; QTimer::singleShot(1500, this, [this]() { browserClosing_ = false; }); } // 置位抑制标记并挂 1.5s 看门狗：CloseBrowser 未能回调 OnBeforeClose 时（CEF windowed 下常见）标记不会永久卡 true，否则标题栏 ✕ 会彻底失效
bool QtAppWindow::nativeEvent(const QByteArray& eventType, void* message, qintptr* result) { MSG* msg = static_cast<MSG*>(message); if (msg->message == WM_APP_COMMAND) { std::unique_ptr<CmdRequest> req(reinterpret_cast<CmdRequest*>(msg->lParam)); if (req) api_.Execute(req->id, req->cmd, req->args_json); *result = 0; return true; } if (msg->message == WM_APP_ENSURE_TAB) { tabs_.EnsureOneTab(); tabs_.LayoutActive(); *result = 0; return true; } if (msg->message == WM_APP_DEFER) { std::unique_ptr<std::function<void()>> task(reinterpret_cast<std::function<void()>*>(msg->lParam)); if (task) (*task)(); *result = 0; return true; } return QMainWindow::nativeEvent(eventType, message, result); } // 接收管道线程三类消息：指令→AgentApi（unique_ptr 接管所有权）、补建标签、UI 线程回调
void QtAppWindow::resizeEvent(QResizeEvent* event) { tabs_.LayoutActive(); UpdateCursorBounds(); QMainWindow::resizeEvent(event); } // 尺寸变化同步 CEF 子窗口位置与光标覆盖层
void QtAppWindow::closeEvent(QCloseEvent* event) { if (quitting_) { event->accept(); return; } event->ignore(); if (browserClosing_) return; HideSelf(); } // ✕ 关闭：非退出流程一律忽略并隐藏保活；关闭标签期间（browserClosing_）CEF 销毁子窗口会连带向宿主发 WM_CLOSE，必须直接忽略否则整窗会被误隐藏

// ---------- 标签页图标 ----------
class FaviconRequestClient : public CefURLRequestClient { // CefURLRequest 客户端：累积下载字节，完成时回传缓冲
public:
    explicit FaviconRequestClient(std::function<void(const std::vector<uint8_t>&)> cb) : cb_(cb) {}
    void OnRequestComplete(CefRefPtr<CefURLRequest> request) override { if (request->GetRequestStatus() != UR_CANCELED) cb_(data_); }
    void OnDownloadData(CefRefPtr<CefURLRequest> request, const void* data, size_t length) override { const uint8_t* p = static_cast<const uint8_t*>(data); data_.insert(data_.end(), p, p + length); }
    void OnUploadProgress(CefRefPtr<CefURLRequest> request, int64 current, int64 total) override {}
    void OnDownloadProgress(CefRefPtr<CefURLRequest> request, int64 current, int64 total) override {}
    bool GetAuthCredentials(bool isProxy, const CefString& host, int port, const CefString& realm, const CefString& scheme, CefRefPtr<CefAuthCallback> callback) override { return false; } // 图标下载无需认证
private:
    std::function<void(const std::vector<uint8_t>&)> cb_;
    std::vector<uint8_t> data_;
    IMPLEMENT_REFCOUNTING(FaviconRequestClient);
    DISALLOW_COPY_AND_ASSIGN(FaviconRequestClient);
};

QIcon QtAppWindow::DefaultIcon() const { // resources/dragon.ico 默认图标（加载中/失败兜底）
    static QIcon icon;
    static bool loaded = false;
    if (!loaded) { icon = QIcon(Config::resolvePath("resources/dragon.ico")); loaded = true; }
    return icon;
}

void QtAppWindow::SetTabIcon(BrowserTab* tab, const QIcon& icon) { // 写回标签图标并按下标刷新标签栏
    if (!tab) return;
    tab->icon = icon;
    for (int i = 0; i < tabs_.count(); ++i) { // TabManager 无 IndexOf，线性定位下标
        if (tabs_.At(i) == tab) { tabsWidget_->setTabIcon(i, icon); break; }
    }
}

void QtAppWindow::FetchFavicon(CefRefPtr<CefBrowser> browser) { // 加载完成后取网页图标
    BrowserTab* tab = tabs_.Find(browser);
    if (!tab || !tab->cdp) return;
    const std::wstring& url = tab->url; // 非 http/https/file 直接用默认图标
    if (url.empty() || (url.rfind(L"http://", 0) != 0 && url.rfind(L"https://", 0) != 0 && url.rfind(L"file://", 0) != 0)) { SetTabIcon(tab, DefaultIcon()); return; }
    SetTabIcon(tab, DefaultIcon()); // 先放默认图标，下载成功后再替换
    std::string js = "(()=>{var el=document.querySelector('link[rel~=\"icon\"]');var h=el?el.href:'';return h||'';})()"; // 读取页面声明的图标地址
    tab->cdp->Evaluate(js, false, [this, browser](bool ok, CefRefPtr<CefValue> value, const std::string& error) { // CDP 执行 JS，回调在 CEF UI 线程；value 为 returnByValue 结果
        std::string href;
        if (ok && value && value->GetType() == VTYPE_STRING) href = value->GetString(); // JS 返回字符串，直接取 CefValue 字符串
        BrowserTab* t = tabs_.Find(browser);
        if (!t) return;
        std::string target;
        if (!href.empty() && (href.rfind("http://", 0) == 0 || href.rfind("https://", 0) == 0)) {
            target = href; // 页面显式声明图标
        } else { // 回退到站点的 /favicon.ico
            const std::string u = WideToUtf8(t->url);
            size_t pos = u.find("://");
            if (pos != std::string::npos) { size_t slash = u.find('/', pos + 3); target = (slash == std::string::npos ? u : u.substr(0, slash)) + "/favicon.ico"; }
        }
        if (target.empty()) { Defer([this, browser]() { SetTabIcon(tabs_.Find(browser), DefaultIcon()); }); return; }
        DownloadFavicon(browser, target);
    });
}

void QtAppWindow::DownloadFavicon(CefRefPtr<CefBrowser> browser, const std::string& url) { // 经 CefURLRequest 下载并解码图标
    CefRefPtr<CefRequest> req = CefRequest::Create();
    req->SetURL(url);
    req->SetMethod("GET");
    CefRefPtr<FaviconRequestClient> client = new FaviconRequestClient([this, browser](const std::vector<uint8_t>& bytes) { // 回调在 CEF UI 线程，解码与刷新经 Defer 回到 Qt UI 线程
        Defer([this, browser, bytes]() {
            BrowserTab* t = tabs_.Find(browser);
            if (!t) return;
            if (bytes.size() > 256 * 1024) { SetTabIcon(t, DefaultIcon()); return; } // 过大按失败兜底
            QImage img;
            if (!bytes.empty() && img.loadFromData(bytes.data(), static_cast<int>(bytes.size()))) {
                const QPixmap pm = QPixmap::fromImage(img.scaled(16, 16, Qt::KeepAspectRatio, Qt::SmoothTransformation)); // 统一缩到 16x16 标签图标
                SetTabIcon(t, QIcon(pm));
            } else {
                SetTabIcon(t, DefaultIcon());
            }
        });
    });
    CefURLRequest::Create(req, client, nullptr);
}
// ---------- 智能体模拟鼠标 ----------
void QtAppWindow::ShowAgentCursor(double x, double y, double vw, double vh, bool click) { // 页面视口坐标 → 覆盖层归一化坐标
    if (cursor_ == nullptr || vw <= 0 || vh <= 0) return; // 视口未知时忽略
    UpdateCursorBounds(); // 先对齐承载区，避免窗口刚移动时坐标偏移
    cursor_->ShowAt(x / vw, y / vh, click); // 归一化并显示；click 播放涟漪
}
void QtAppWindow::HideAgentCursor() { if (cursor_ != nullptr) cursor_->HideCursor(); } // 隐藏模拟鼠标
void QtAppWindow::UpdateCursorBounds() { // 覆盖层与 CEF 承载区保持同一全局矩形
    if (cursor_ == nullptr || cefHost_ == nullptr) return;
    const QPoint top_left = cefHost_->mapToGlobal(QPoint(0, 0)); // 承载区左上角屏幕坐标
    cursor_->SetHostGeometry(QRect(top_left, cefHost_->size())); // 逻辑尺寸与 Qt 窗口一致
}
void QtAppWindow::moveEvent(QMoveEvent* event) { UpdateCursorBounds(); QMainWindow::moveEvent(event); } // 窗口移动时同步光标覆盖层
