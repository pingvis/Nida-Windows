// Windows-only CEF OSR host. stdout is exclusively the Nida binary protocol.
#include <windows.h>
#include <fcntl.h>
#include <io.h>
#include <algorithm>
#include <array>
#include <condition_variable>
#include <cstdint>
#include <cstdio>
#include <future>
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <vector>
#include "include/cef_app.h"
#include "include/cef_browser.h"
#include "include/cef_client.h"
#include "include/cef_parser.h"
#include "include/cef_sandbox_win.h"
#include "include/cef_task.h"

namespace {
constexpr int kMaxDimension = 4096;
constexpr size_t kMaxSources = 16;
struct Packet {
  uint64_t id = 0, generation = 0;
  uint32_t width = 0, height = 0;
  bool error = false;
  std::vector<uint8_t> data;
};
std::mutex output_mutex;
std::condition_variable output_ready;
// At most one frame and one status per browser, irrespective of render rate.
std::map<uint64_t, Packet> pending_frames, pending_status;
bool output_stopped = false;

void Publish(Packet packet) {
  std::unique_lock lock(output_mutex, std::defer_lock);
  if (packet.error) lock.lock();
  else if (!lock.try_lock()) return;
  if (output_stopped) return;
  auto& pending = packet.error ? pending_status : pending_frames;
  pending.insert_or_assign(packet.id, std::move(packet));
  output_ready.notify_one();
}
void Status(uint64_t id, uint64_t generation, const std::string& text) {
  Packet packet;
  packet.id = id; packet.generation = generation; packet.error = true;
  packet.data.assign(text.begin(), text.end());
  Publish(std::move(packet));
}
void Put32(std::array<uint8_t, 32>& h, size_t at, uint32_t n) {
  for (int i = 0; i < 4; ++i) h[at + i] = static_cast<uint8_t>(n >> (i * 8));
}
void Put64(std::array<uint8_t, 32>& h, size_t at, uint64_t n) {
  for (int i = 0; i < 8; ++i) h[at + i] = static_cast<uint8_t>(n >> (i * 8));
}
void WritePackets() {
  for (;;) {
    std::map<uint64_t, Packet> frames, statuses;
    {
      std::unique_lock lock(output_mutex);
      output_ready.wait(lock, [] { return output_stopped || !pending_frames.empty() || !pending_status.empty(); });
      if (output_stopped) return;
      frames.swap(pending_frames); statuses.swap(pending_status);
    }
    for (auto* packets : {&statuses, &frames}) for (auto& [id, p] : *packets) {
      std::array<uint8_t, 32> header{'N', 'D', static_cast<uint8_t>(p.error ? 'E' : 'B'), '1'};
      Put64(header, 4, id); Put64(header, 12, p.generation);
      Put32(header, 20, p.width); Put32(header, 24, p.height);
      Put32(header, 28, static_cast<uint32_t>(p.data.size()));
      if (fwrite(header.data(), 1, header.size(), stdout) != header.size() ||
          fwrite(p.data.data(), 1, p.data.size(), stdout) != p.data.size() || fflush(stdout)) return;
    }
  }
}

class View;
std::map<uint64_t, CefRefPtr<View>> views; // CEF UI thread only.
bool closing = false;

class View final : public CefClient, public CefRenderHandler,
                   public CefLifeSpanHandler, public CefLoadHandler,
                   public CefRequestHandler, public CefJSDialogHandler {
 public:
  uint64_t id = 0, generation = 0;
  int width = 1920, height = 1080, fps = 30;
  std::string url;
  CefRefPtr<CefBrowser> browser;
  bool stopped = false;
  CefRefPtr<CefRenderHandler> GetRenderHandler() override { return this; }
  CefRefPtr<CefLifeSpanHandler> GetLifeSpanHandler() override { return this; }
  CefRefPtr<CefLoadHandler> GetLoadHandler() override { return this; }
  CefRefPtr<CefRequestHandler> GetRequestHandler() override { return this; }
  CefRefPtr<CefJSDialogHandler> GetJSDialogHandler() override { return this; }
  void GetViewRect(CefRefPtr<CefBrowser>, CefRect& rect) override { rect = CefRect(0, 0, width, height); }
  bool GetScreenInfo(CefRefPtr<CefBrowser>, CefScreenInfo& info) override {
    info.device_scale_factor = 1.0;
    info.rect = info.available_rect = CefRect(0, 0, width, height);
    return true;
  }
  void OnAfterCreated(CefRefPtr<CefBrowser> b) override {
    browser = b;
    if (stopped || closing) { b->GetHost()->CloseBrowser(true); return; }
    b->GetHost()->SetAudioMuted(true);
    b->GetHost()->SetWindowlessFrameRate(fps);
    b->GetHost()->WasResized();
    b->GetMainFrame()->LoadURL(url);
  }
  void OnBeforeClose(CefRefPtr<CefBrowser>) override {
    browser = nullptr;
    auto it = views.find(id);
    if (it != views.end() && it->second.get() == this) views.erase(it);
    if (closing && views.empty()) CefQuitMessageLoop();
  }
  void OnPaint(CefRefPtr<CefBrowser>, PaintElementType type, const RectList&,
               const void* buffer, int w, int h) override {
    if (stopped || type != PET_VIEW || w != width || h != height) return;
    Packet packet;
    packet.id = id; packet.generation = generation;
    packet.width = w; packet.height = h;
    const auto* bytes = static_cast<const uint8_t*>(buffer);
    packet.data.assign(bytes, bytes + static_cast<size_t>(w) * h * 4);
    Publish(std::move(packet));
  }
  void OnLoadEnd(CefRefPtr<CefBrowser>, CefRefPtr<CefFrame> frame, int status) override {
    if (frame->IsMain()) Status(id, generation, status >= 400 ? "Website returned an HTTP error. Refresh to retry." : "");
  }
  void OnLoadError(CefRefPtr<CefBrowser>, CefRefPtr<CefFrame> frame, ErrorCode code,
                   const CefString&, const CefString&) override {
    if (frame->IsMain() && code != ERR_ABORTED)
      Status(id, generation, "Website could not load. Check the URL and connection, then refresh.");
  }
  void OnRenderProcessTerminated(CefRefPtr<CefBrowser>, TerminationStatus,
                                 int, const CefString&) override {
    Status(id, generation, "Website renderer stopped. Refresh to retry.");
  }
  // Browser sources never open desktop windows or hand URLs to external apps.
  bool OnBeforePopup(CefRefPtr<CefBrowser>, CefRefPtr<CefFrame>, int,
                     const CefString&, const CefString&, WindowOpenDisposition, bool,
                     const CefPopupFeatures&, CefWindowInfo&, CefRefPtr<CefClient>&,
                     CefBrowserSettings&, CefRefPtr<CefDictionaryValue>&, bool*) override { return true; }
  bool OnJSDialog(CefRefPtr<CefBrowser>, const CefString&, JSDialogType,
                  const CefString&, const CefString&, CefRefPtr<CefJSDialogCallback>, bool& suppress) override {
    suppress = true; return false;
  }
  bool OnBeforeUnloadDialog(CefRefPtr<CefBrowser>, const CefString&, bool,
                            CefRefPtr<CefJSDialogCallback> callback) override { callback->Continue(true, CefString()); return true; }
 private:
  IMPLEMENT_REFCOUNTING(View);
};

uint64_t Number(CefRefPtr<CefDictionaryValue> d, const char* key) {
  return d->GetType(key) == VTYPE_INT ? static_cast<uint64_t>(d->GetInt(key)) : static_cast<uint64_t>(d->GetDouble(key));
}
class ApplyCommand final : public CefTask {
 public:
  ApplyCommand(CefRefPtr<CefDictionaryValue> d, std::shared_ptr<std::promise<void>> done)
      : command_(d), done_(std::move(done)) {}
  void Execute() override {
    Apply(); done_->set_value();
  }
 private:
  void Apply() {
    const std::string op = command_->GetString("op");
    if (op == "shutdown") {
      closing = true;
      // CloseBrowser can invoke callbacks; don't iterate a map that callbacks erase.
      auto copy = views;
      for (auto& [id, view] : copy) { view->stopped = true; if (view->browser) view->browser->GetHost()->CloseBrowser(true); }
      if (views.empty()) CefQuitMessageLoop();
      return;
    }
    const uint64_t id = Number(command_, "id");
    auto it = views.find(id);
    if (op == "stop") {
      if (it != views.end()) { it->second->stopped = true; if (it->second->browser) it->second->browser->GetHost()->CloseBrowser(true); }
      return;
    }
    if (op == "reload") {
      if (it != views.end() && it->second->browser) it->second->browser->ReloadIgnoreCache();
      return;
    }
    if (op != "configure") return;
    const uint64_t generation = Number(command_, "generation");
    const int width = command_->GetInt("width"), height = command_->GetInt("height"), fps = command_->GetInt("fps");
    const std::string url = command_->GetString("url");
    if (width < 1 || height < 1 || width > kMaxDimension || height > kMaxDimension || fps < 1 || fps > 60 || url.size() > 65536) {
      Status(id, generation, "Invalid browser settings"); return;
    }
    CefRefPtr<View> view;
    const bool fresh = it == views.end();
    if (fresh) {
      if (views.size() >= kMaxSources) { Status(id, generation, "Browser source limit reached (16)"); return; }
      view = new View; view->id = id;
      views.emplace(id, view);
    } else { view = it->second; }
    const bool navigate = view->url != url;
    view->generation = generation; view->url = url;
    view->width = width; view->height = height; view->fps = fps;
    if (fresh) {
      CefWindowInfo window; window.SetAsWindowless(nullptr);
      CefBrowserSettings settings; settings.background_color = CefColorSetARGB(0, 0, 0, 0);
      settings.windowless_frame_rate = fps;
      if (!CefBrowserHost::CreateBrowser(window, view, "about:blank", settings, nullptr, nullptr)) {
        views.erase(id); Status(id, generation, "Could not create browser source");
      }
    } else if (view->browser) {
      view->browser->GetHost()->SetWindowlessFrameRate(fps);
      view->browser->GetHost()->WasResized();
      if (navigate) view->browser->GetMainFrame()->LoadURL(url);
      view->browser->GetHost()->Invalidate(PET_VIEW);
    }
  }
  CefRefPtr<CefDictionaryValue> command_;
  std::shared_ptr<std::promise<void>> done_;
  IMPLEMENT_REFCOUNTING(ApplyCommand);
};
void Dispatch(CefRefPtr<CefDictionaryValue> command) {
  auto done = std::make_shared<std::promise<void>>();
  auto future = done->get_future();
  if (CefPostTask(TID_UI, new ApplyCommand(command, done))) future.wait();
}
void ReadCommands() {
  for (;;) {
    uint8_t header[4];
    if (fread(header, 1, 4, stdin) != 4) break;
    const uint32_t size = uint32_t(header[0]) | uint32_t(header[1]) << 8 | uint32_t(header[2]) << 16 | uint32_t(header[3]) << 24;
    if (size > 262144) break;
    std::string data(size, '\0');
    if (fread(data.data(), 1, size, stdin) != size) break;
    auto value = CefParseJSON(data, JSON_PARSER_RFC);
    if (!value || value->GetType() != VTYPE_DICTIONARY) break;
    Dispatch(value->GetDictionary());
  }
  auto command = CefDictionaryValue::Create(); command->SetString("op", "shutdown"); Dispatch(command);
}
class Application final : public CefApp {
 public:
  void OnBeforeCommandLineProcessing(const CefString&, CefRefPtr<CefCommandLine> command) override {
    // Off-air sources retain their live chat session and JS timers.
    command->AppendSwitch("disable-background-timer-throttling");
    command->AppendSwitch("disable-renderer-backgrounding");
  }
 private:
  IMPLEMENT_REFCOUNTING(Application);
};
} // namespace

CEF_BOOTSTRAP_EXPORT int RunWinMain(HINSTANCE instance, LPWSTR, int, void* sandbox_info, cef_version_info_t*) {
  CefMainArgs args(instance);
  CefRefPtr<Application> app = new Application;
  const int code = CefExecuteProcess(args, app, sandbox_info);
  if (code >= 0) return code;
  _setmode(_fileno(stdin), _O_BINARY); _setmode(_fileno(stdout), _O_BINARY);
  CefSettings settings;
  settings.windowless_rendering_enabled = true;
  settings.background_color = CefColorSetARGB(0, 0, 0, 0);
  settings.log_severity = LOGSEVERITY_DISABLE; // Overlay URLs may carry tokens.
  auto command = CefCommandLine::CreateCommandLine(); command->InitFromString(GetCommandLineW());
  const auto cache = command->GetSwitchValue("cache-path");
  if (cache.empty()) return 2;
  CefString(&settings.root_cache_path) = cache;
  CefString(&settings.cache_path) = cache.ToWString() + L"\\Default";
  settings.persist_session_cookies = true;
  if (!CefInitialize(args, settings, app, sandbox_info)) return 3;
  std::thread writer(WritePackets), reader(ReadCommands);
  CefRunMessageLoop();
  reader.join();
  {
    std::lock_guard lock(output_mutex); output_stopped = true;
  }
  output_ready.notify_one(); writer.join();
  CefShutdown();
  return 0;
}
