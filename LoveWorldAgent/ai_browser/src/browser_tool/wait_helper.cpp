// WaitForHelper 实现：有界主框架导航等待 + 弹窗短接 + 100ms 导航探针 + MutationObserver DOM 静默等待
#include "browser_tool/wait_helper.h"
#include <memory>
#include <QTimer>
#include "include/wrapper/cef_helpers.h"
#include "server/app_handler.h"
#include "server/cdp_client.h"
#include "uis/app_window.h"
#include "uis/tab_manager.h"
namespace {
const int kNavSettleDelayMs = 150;
const int kProbeMs = 100;
const int kProbeNavCapMs = 5000;
const char* kDomSettleJs = R"JS((function(){
return new Promise(function(resolve){
var root=document.body||document.documentElement;
if(!root){resolve('empty');return;}
var done=false,qt=null,ct=null;
function finish(r){
if(done){return;}
done=true;
if(qt){clearTimeout(qt);}
if(ct){clearTimeout(ct);}
try{obs.disconnect();}catch(e){}
resolve(r);
}
var obs=new MutationObserver(function(muts){
for(var i=0;i<muts.length;i++){
if(muts[i].type==='attributes'&&muts[i].attributeName==='data-lwa-ref'){continue;}
if(qt){clearTimeout(qt);}
qt=setTimeout(function(){finish('quiet');},150);
break;
}
});
try{obs.observe(root,{childList:true,subtree:true,attributes:true});}catch(e){finish('noobs');return;}
qt=setTimeout(function(){finish('quiet');},150);
ct=setTimeout(function(){finish('cap');},1200);
});
})())JS";

void RunDomSettle(const CefRefPtr<AppHandler>& handler, const CefRefPtr<CdpClient>& cdp, std::function<void(bool)> done) {
    if (handler == nullptr) {
        if (done) done(false);
        return;
    }
    if (handler->dialog_pending()) {
        if (done) done(true);
        return;
    }
    if (cdp == nullptr) {
        if (done) done(false);
        return;
    }
    auto finished = std::make_shared<bool>(false);
    auto finish = [finished, handler, done](bool dlg) {
        if (*finished) return;
        *finished = true;
        const bool is_dlg = dlg || (handler != nullptr && handler->dialog_pending());
        if (done) done(is_dlg);
    };
    // 若在 DOM 等待期间弹出 alert/confirm，立即短接，避免 Evaluate 挂起
    handler->AddNavWaiter([finish, handler]() {
        if (handler != nullptr && handler->dialog_pending()) {
            finish(true);
        }
    });
    cdp->Evaluate(kDomSettleJs, true, [finish](bool, CefRefPtr<CefValue>, const std::string&) {
        finish(false);
    });
}
}  // namespace

void WaitForHelper::WaitForNavigation(BrowserTab* tab, uint64_t prev_seq, int timeout_ms, std::function<void()> done) {
    CEF_REQUIRE_UI_THREAD();
    CefRefPtr<AppHandler> handler = tab != nullptr ? tab->handler : nullptr;
    WaitForNavigation(handler, prev_seq, timeout_ms, std::move(done));
}

void WaitForHelper::WaitForNavigation(CefRefPtr<AppHandler> handler, uint64_t prev_seq, int timeout_ms, std::function<void()> done) {
    CEF_REQUIRE_UI_THREAD();
    if (handler == nullptr) {
        if (done) done();
        return;
    }
    if (handler->dialog_pending()) {
        if (done) done();
        return;
    }
    if (handler->nav_seq() > prev_seq) {
        QTimer::singleShot(kNavSettleDelayMs, [done]() { if (done) done(); });
        return;
    }
    auto finished = std::make_shared<bool>(false);
    auto complete = [finished, handler, done](bool immediate) {
        if (*finished) return;
        *finished = true;
        if (immediate || handler->dialog_pending()) {
            if (done) done();
            return;
        }
        QTimer::singleShot(kNavSettleDelayMs, [done]() { if (done) done(); });
    };
    handler->AddNavWaiter([complete, handler, prev_seq]() {
        if (handler->dialog_pending()) {
            complete(true);
            return;
        }
        if (handler->nav_seq() > prev_seq) {
            complete(false);
        }
    });
    QTimer::singleShot(timeout_ms, [complete]() { complete(true); });
}

void WaitForHelper::Settle(const ToolRequest& req, bool maybe_navigates, std::function<void(bool dialog_pending)> done) {
    CEF_REQUIRE_UI_THREAD();
    if (req.tab == nullptr || req.tab->handler == nullptr) {
        if (done) done(false);
        return;
    }
    CefRefPtr<AppHandler> handler = req.tab->handler;
    CefRefPtr<CefBrowser> browser = req.tab->browser;
    CefRefPtr<CdpClient> cdp = req.cdp;
    AppWindow* window = req.window;
    if (handler->dialog_pending()) {
        if (done) done(true);
        return;
    }
    if (!maybe_navigates) {
        RunDomSettle(handler, cdp, std::move(done));
        return;
    }
    const uint64_t prev_seq = handler->nav_seq();
    QTimer::singleShot(kProbeMs, [window, browser, handler, cdp, prev_seq, done]() {
        BrowserTab* cur = (window != nullptr && window->Tabs() != nullptr && browser != nullptr) ? window->Tabs()->Find(browser) : nullptr;
        if (cur == nullptr) {
            if (done) done(handler != nullptr && handler->dialog_pending());
            return;
        }
        if (handler->dialog_pending()) {
            if (done) done(true);
            return;
        }
        if (cur->loading || handler->nav_seq() > prev_seq) {
            WaitForNavigation(handler, prev_seq, kProbeNavCapMs, [window, browser, handler, cdp, done]() {
                BrowserTab* cur_after = (window != nullptr && window->Tabs() != nullptr && browser != nullptr) ? window->Tabs()->Find(browser) : nullptr;
                if (cur_after == nullptr) {
                    if (done) done(handler != nullptr && handler->dialog_pending());
                    return;
                }
                RunDomSettle(handler, cur_after->cdp != nullptr ? cur_after->cdp : cdp, done);
            });
            return;
        }
        RunDomSettle(handler, cur->cdp != nullptr ? cur->cdp : cdp, done);
    });
}
