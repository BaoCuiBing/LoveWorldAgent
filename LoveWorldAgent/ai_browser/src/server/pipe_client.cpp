// PipeClient 实现：管道读写、协议 v2 帧解析、响应分片出站
#include "server/pipe_client.h" // 自身声明
#include <chrono> // wait_for 时长
#include <cstdio> // 标准输入输出
#include <cstdlib> // 标准库
#include "uis/app_window.h" // AppWindow 接口
#include "server/client_app.h" // ClientApp::PanelHandle
#include "utils/json_util.h" // JSON 工具
#include "utils/logging.h" // 文件日志
namespace {
const DWORD kConnectRetryMs = 500; // 连接失败重试间隔
const DWORD kReadChunk = 65536; // 单次 ReadFile 读块大小
const size_t kMaxFrameBytes = 32768; // 单帧 data 文本上限：分片后每帧远小于管道缓冲，写线程被背压时不会卡住 UI 线程
std::string BuildRes(int id, bool ok, CefRefPtr<CefValue> data, const std::string& error, int chunk, int chunks, const std::string& format = "") { CefRefPtr<CefDictionaryValue> frame = MakeDict(); frame->SetString("type", "res"); frame->SetInt("id", id); frame->SetBool("ok", ok); if (chunks > 1) { frame->SetInt("chunks", chunks); frame->SetInt("chunk", chunk); if (!format.empty()) frame->SetString("format", format); } if (data != nullptr) frame->SetValue("data", data); if (!error.empty()) frame->SetString("error", error); return WriteJsonDict(frame); } // 组装 res 帧
}
PipeClient* PipeClient::instance_ = nullptr; // 单例初始化
std::mutex PipeClient::life_lock_; // 生命周期锁初始化
void PipeClient::Start(const std::wstring& pipe_name) { std::lock_guard<std::mutex> lock(life_lock_); if (instance_ != nullptr) return; instance_ = new PipeClient(pipe_name); instance_->running_ = true; PipeClient* self = instance_; self->reader_ = std::thread([self] { self->ReadLoop(); }); self->writer_ = std::thread([self] { self->WriteLoop(); }); LOG_INFO("pipe", QStringLiteral("管道客户端启动：%1").arg(QString::fromStdWString(pipe_name))); if (ClientApp::PanelHandle() != nullptr && ::IsWindow(ClientApp::PanelHandle())) self->DispatchQueued(ClientApp::PanelHandle()); } // 启动读写线程；OnContextInitialized 早于本函数时 SetTarget 被丢弃，这里补设目标并冲入排队指令
void PipeClient::Stop() { std::lock_guard<std::mutex> lock(life_lock_); if (instance_ == nullptr) return; LOG_INFO("pipe", QStringLiteral("管道客户端停止")); instance_->running_ = false; instance_->out_cv_.notify_all(); { std::lock_guard<std::mutex> io_lock(instance_->io_lock_); if (instance_->pipe_ != INVALID_HANDLE_VALUE) ::CancelIoEx(instance_->pipe_, nullptr); if (instance_->write_ != INVALID_HANDLE_VALUE) ::CancelIoEx(instance_->write_, nullptr); } if (instance_->reader_.joinable()) instance_->reader_.join(); if (instance_->writer_.joinable()) instance_->writer_.join(); delete instance_; instance_ = nullptr; } // 停止：取消阻塞 IO、汇合线程、销毁单例
bool PipeClient::Alive() { std::lock_guard<std::mutex> lock(life_lock_); return instance_ != nullptr && instance_->running_; } // 是否运行中
void PipeClient::SetTarget(HWND target) { std::lock_guard<std::mutex> lock(life_lock_); if (instance_ == nullptr) return; instance_->DispatchQueued(target); } // 设定目标并冲入排队指令
void PipeClient::Respond(int id, CefRefPtr<CefDictionaryValue> data) {
    std::lock_guard<std::mutex> lock(life_lock_);
    if (instance_ == nullptr || id == 0) return;
    if (data == nullptr) { instance_->Write(BuildRes(id, true, nullptr, "", 0, 1)); return; }
    const std::string raw = WriteJsonDict(data);
    if (raw.size() <= kMaxFrameBytes) {
        instance_->Write(BuildRes(id, true, DictValue(data), "", 0, 1));
        return;
    }
    std::vector<std::string> parts;
    size_t pos = 0;
    while (pos < raw.size()) {
        size_t next = pos;
        parts.push_back(Utf8Slice(raw, pos, kMaxFrameBytes, next));
        if (next <= pos) break;
        pos = next;
    }
    const int total = static_cast<int>(parts.size());
    for (int i = 0; i < total; ++i) instance_->Write(BuildRes(id, true, StringValue(parts[i]), "", i, total, "json"));
} // 响应成功；超限自动按 UTF-8 边界分片并标记 format=json
void PipeClient::RespondError(int id, const std::string& error) { std::lock_guard<std::mutex> lock(life_lock_); if (instance_ == nullptr || id == 0) return; instance_->Write(BuildRes(id, false, nullptr, error, 0, 1)); } // 响应失败
void PipeClient::RespondText(int id, const std::string& text) { std::lock_guard<std::mutex> lock(life_lock_); if (instance_ == nullptr || id == 0) return; if (text.size() <= kMaxFrameBytes) { instance_->Write(BuildRes(id, true, StringValue(text), "", 0, 1)); return; } std::vector<std::string> parts; size_t pos = 0; while (pos < text.size()) { size_t next = pos; parts.push_back(Utf8Slice(text, pos, kMaxFrameBytes, next)); if (next <= pos) break; pos = next; } const int total = static_cast<int>(parts.size()); for (int i = 0; i < total; ++i) instance_->Write(BuildRes(id, true, StringValue(parts[i]), "", i, total)); } // 按 UTF-8 边界切片后逐片下发，接收端按 chunk 顺序拼接
void PipeClient::PostEvent(const std::string& event, CefRefPtr<CefDictionaryValue> data) { std::lock_guard<std::mutex> lock(life_lock_); if (instance_ == nullptr) return; CefRefPtr<CefDictionaryValue> frame = MakeDict(); frame->SetString("type", "ev"); frame->SetString("event", event); frame->SetDictionary("data", data != nullptr ? data : MakeDict()); instance_->Write(WriteJsonDict(frame)); } // 组装 ev 帧入出站队列
PipeClient::PipeClient(std::wstring pipe_name) : pipe_name_(std::move(pipe_name)) {} // 构造保存管道名
PipeClient::~PipeClient() { CloseHandles(); } // 析构关闭句柄
void PipeClient::DispatchQueued(HWND target) { std::vector<std::unique_ptr<CmdRequest>> queued; { std::lock_guard<std::mutex> lock(io_lock_); target_ = target; queued.swap(pending_); } for (auto& req : queued) PostRequest(target, std::move(req)); } // 目标就绪后冲入排队指令
void PipeClient::PostRequest(HWND target, std::unique_ptr<CmdRequest> req) { if (target == nullptr || req == nullptr || !::IsWindow(target)) return; ::PostMessageW(target, WM_APP_COMMAND, 0, reinterpret_cast<LPARAM>(req.release())); } // 消息队列跨线程传递所有权，接收方（AppWindow::nativeEvent）负责释放
bool PipeClient::OpenOnce() { const std::wstring full = L"\\\\.\\pipe\\" + pipe_name_; const HANDLE handle = ::CreateFileW(full.c_str(), GENERIC_READ | GENERIC_WRITE, 0, nullptr, OPEN_EXISTING, 0, nullptr); if (handle == INVALID_HANDLE_VALUE) return false; HANDLE dup = INVALID_HANDLE_VALUE; const BOOL ok = ::DuplicateHandle(::GetCurrentProcess(), handle, ::GetCurrentProcess(), &dup, 0, FALSE, DUPLICATE_SAME_ACCESS); if (!ok) dup = handle; std::lock_guard<std::mutex> lock(io_lock_); pipe_ = handle; write_ = dup; LOG_INFO("pipe", QStringLiteral("已连接主应用管道：%1").arg(QString::fromStdWString(full))); return true; } // 连接管道并复制一份句柄给写线程：读写各自用各自句柄，写阻塞（管道满）不会拖住读线程
void PipeClient::CloseHandles() { HANDLE read = INVALID_HANDLE_VALUE; HANDLE dup = INVALID_HANDLE_VALUE; { std::lock_guard<std::mutex> lock(io_lock_); read = pipe_; dup = write_; pipe_ = INVALID_HANDLE_VALUE; write_ = INVALID_HANDLE_VALUE; } if (read != INVALID_HANDLE_VALUE) ::CloseHandle(read); if (dup != INVALID_HANDLE_VALUE && dup != read) ::CloseHandle(dup); } // 关闭读写句柄（同一句柄只关一次）
void PipeClient::ReadLoop() {
    std::string buffer;
    while (running_) {
        if (!OpenOnce()) { ::Sleep(kConnectRetryMs); continue; }
        buffer.clear();
        ULONGLONG last_active = ::GetTickCount64();
        while (running_) {
            DWORD available = 0;
            HANDLE read_handle = INVALID_HANDLE_VALUE;
            { std::lock_guard<std::mutex> lock(io_lock_); read_handle = pipe_; }
            if (read_handle == INVALID_HANDLE_VALUE || !::PeekNamedPipe(read_handle, nullptr, 0, nullptr, &available, nullptr)) break;
            if (available > 0) {
                char chunk[kReadChunk] = {0};
                DWORD read = 0;
                const DWORD want = available > kReadChunk ? kReadChunk : available;
                if (!::ReadFile(read_handle, chunk, want, &read, nullptr)) {
                    if (::GetLastError() == ERROR_MORE_DATA && read > 0) {
                        last_active = ::GetTickCount64();
                        buffer.append(chunk, read);
                        continue;
                    }
                    break;
                }
                if (read == 0) break;
                last_active = ::GetTickCount64();
                buffer.append(chunk, read);
            } else {
                const ULONGLONG idle = ::GetTickCount64() - last_active;
                const DWORD sleep_ms = idle < 200 ? 1 : (idle < 800 ? 2 : (idle < 2500 ? 5 : 15));
                ::Sleep(sleep_ms);
            }
            size_t newline = buffer.find('\n');
            while (newline != std::string::npos) {
                std::string line = buffer.substr(0, newline);
                buffer.erase(0, newline + 1);
                if (!line.empty() && line.back() == '\r') line.pop_back();
                if (!line.empty()) HandleLine(line);
                newline = buffer.find('\n');
            }
        }
        CloseHandles();
        if (running_) ::Sleep(kConnectRetryMs);
    }
} // 读线程：ERROR_MORE_DATA 续读 + 自适应退避轮询（1ms->15ms），按行分发后断线重连
void PipeClient::HandleLine(const std::string& line) { CefRefPtr<CefDictionaryValue> frame = ParseJsonObject(line); if (frame == nullptr) { LOG_WARN("pipe", QStringLiteral("忽略无法解析的帧：%1").arg(QString::fromStdString(line.substr(0, 120)))); return; } const std::string type = StrOf(frame, "type"); std::unique_ptr<CmdRequest> req(new CmdRequest()); if (type == "req") { req->id = IntOf(frame, "id"); req->cmd = StrOf(frame, "cmd"); CefRefPtr<CefDictionaryValue> args = ArgGetDict(frame, "args"); req->args_json = args != nullptr ? WriteJsonDict(args) : std::string("{}"); } else if (type == "navigate") { req->cmd = "browser_navigate"; CefRefPtr<CefDictionaryValue> args = MakeDict(); args->SetString("url", StrOf(frame, "url")); req->args_json = WriteJsonDict(args); } else if (type == "setRect") { req->cmd = "window_rect"; CefRefPtr<CefDictionaryValue> args = MakeDict(); args->SetString("rect", StrOf(frame, "rect")); req->args_json = WriteJsonDict(args); } else if (type == "show") { req->cmd = "window_show"; CefRefPtr<CefDictionaryValue> args = MakeDict(); args->SetBool("value", BoolOf(frame, "value")); req->args_json = WriteJsonDict(args); } else { LOG_WARN("pipe", QStringLiteral("忽略未知帧类型：%1").arg(QString::fromStdString(type))); return; } if (req->cmd.empty()) { LOG_WARN("pipe", QStringLiteral("帧缺少 cmd 字段，已丢弃")); return; } HWND target = nullptr; { std::lock_guard<std::mutex> lock(io_lock_); target = target_; if (target == nullptr) { pending_.push_back(std::move(req)); return; } } PostRequest(target, std::move(req)); } // 单行帧解析：navigate/setRect/show 为兼容旧帧（等价于 id=0 的一次性指令），目标未就绪时排队
void PipeClient::Write(const std::string& line) { std::lock_guard<std::mutex> lock(out_lock_); outbox_.push_back(line); out_cv_.notify_one(); } // 只入队：管道满时 WriteFile 会长时间阻塞，绝不能发生在 CEF UI 线程上
void PipeClient::WriteLoop() { while (running_) { std::string line; { std::unique_lock<std::mutex> lock(out_lock_); out_cv_.wait_for(lock, std::chrono::milliseconds(100), [this] { return !running_ || !outbox_.empty(); }); if (!running_) return; if (outbox_.empty()) continue; line = outbox_.front(); outbox_.erase(outbox_.begin()); } Flush(line); } } // 写线程：出站队列逐条落管道
void PipeClient::Flush(const std::string& line) {
    std::lock_guard<std::mutex> lock(write_lock_);
    HANDLE handle = INVALID_HANDLE_VALUE;
    { std::lock_guard<std::mutex> io_lock(io_lock_); handle = write_; }
    if (handle == INVALID_HANDLE_VALUE || !running_) return;
    const std::string payload = line + "\n";
    size_t offset = 0;
    while (offset < payload.size() && running_) {
        DWORD written = 0;
        if (!::WriteFile(handle, payload.data() + offset, static_cast<DWORD>(payload.size() - offset), &written, nullptr) || written == 0) {
            LOG_WARN("pipe", QStringLiteral("帧写管道失败，错误码=%1").arg(::GetLastError()));
            break;
        }
        offset += written;
    }
} // 写线程专用句柄 write_：循环写完整帧，避免大帧部分写截断
