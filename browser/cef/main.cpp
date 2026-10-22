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
#include <filesystem>
#include <fstream>
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
  // The writer only swaps maps under this lock; pipe writes happen outside it.
  // Never drop a static page's sole paint callback because a swap is in flight.
  std::unique_lock lock(output_mutex);
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
bool context_ready = false;
std::wstring credits_output;
bool credits_ok = false;

class CreditsWriter final : public CefStringVisitor {
 public:
  explicit CreditsWriter(CefRefPtr<CefBrowser> browser) : browser_(browser) {}
  void Visit(const CefString& source) override {
    const std::string html = source.ToString();
    std::ofstream file(std::filesystem::path(credits_output), std::ios::binary);
    file.write(html.data(), static_cast<std::streamsize>(html.size()));
    file.close();
    credits_ok = file.good() && html.size() > 10000;
    closing = true;
    browser_->GetHost()->CloseBrowser(true);
  }
 private:
  CefRefPtr<CefBrowser> browser_;
  IMPLEMENT_REFCOUNTING(CreditsWriter);
};

class View final : public CefClient, public CefRenderHandler,
                   public CefLifeSpanHandler, public CefLoadHandler,
                   public CefRequestHandler, public CefJSDialogHandler {
 public:
  uint64_t id = 0, generation = 0;
  int width = 1920, height = 1080, fps = 30;
  std::string url;
  CefRefPtr<CefBrowser> browser;
  bool stopped = false;
  uint32_t input_modifiers = 0, mouse_buttons = 0;
  uint64_t last_click_time = 0;
  int last_click_x = 0, last_click_y = 0, last_button = -1, click_count = 1;
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
    if (!credits_output.empty() || stopped || type != PET_VIEW || w != width || h != height) return;
    Packet packet;
    packet.id = id; packet.generation = generation;
    packet.width = w; packet.height = h;
    const auto* bytes = static_cast<const uint8_t*>(buffer);
    packet.data.assign(bytes, bytes + static_cast<size_t>(w) * h * 4);
    Publish(std::move(packet));
  }
  void OnLoadEnd(CefRefPtr<CefBrowser> source_browser, CefRefPtr<CefFrame> frame, int status) override {
    if (!credits_output.empty()) {
      if (frame->IsMain() && frame->GetURL().ToString().rfind("chrome://credits", 0) == 0)
        frame->GetSource(new CreditsWriter(source_browser));
      return;
    }
    if (frame->IsMain()) {
      Status(id, generation, status >= 400 ? "Website returned an HTTP error. Refresh to retry." : "");
      // YouTube's standalone chat can mix a white background token with its
      // older dark-theme text tokens. Repair only that white-on-white case,
      // using the page's own background variable. Other sites and transparent
      // overlay feeds retain their authored styles.
      if (status < 400) frame->ExecuteJavaScript(R"JS((() => {
        if (!['www.youtube.com', 'youtube.com'].includes(location.hostname) ||
            !['/live_chat', '/live_chat_replay'].includes(location.pathname)) return;
        const repair = () => {
          const chat = document.querySelector('yt-live-chat-renderer');
          if (!chat) return false;
          if (chat.hasAttribute('is-overlay-feed')) return true;
          const style = getComputedStyle(chat);
          if (style.color === 'rgb(255, 255, 255)' &&
              style.backgroundColor === 'rgb(255, 255, 255)' &&
              style.getPropertyValue('--yt-live-chat-background-color').trim()) {
            chat.style.backgroundColor = 'var(--yt-live-chat-background-color)';
          }
          return true;
        };
        if (!repair()) {
          const observer = new MutationObserver(() => { if (repair()) observer.disconnect(); });
          observer.observe(document.documentElement, {childList: true, subtree: true});
          setTimeout(() => observer.disconnect(), 15000);
        }
      })())JS", frame->GetURL(), 0);
    }
  }
  void OnLoadError(CefRefPtr<CefBrowser> source_browser, CefRefPtr<CefFrame> frame, ErrorCode code,
                   const CefString&, const CefString&) override {
    if (!credits_output.empty() && frame->IsMain() && code != ERR_ABORTED) {
      closing = true; source_browser->GetHost()->CloseBrowser(true); return;
    }
    if (frame->IsMain() && code != ERR_ABORTED)
      Status(id, generation, "Website could not load. Check the URL and connection, then refresh.");
  }
  void OnRenderProcessTerminated(CefRefPtr<CefBrowser>, TerminationStatus,
                                 int, const CefString&) override {
    Status(id, generation, "Website renderer stopped. Refresh to retry.");
  }
  // Browser sources never open desktop windows or hand URLs to external apps.
  bool OnBeforePopup(CefRefPtr<CefBrowser>, CefRefPtr<CefFrame>, int,
                     const CefString&, const CefString&, CefLifeSpanHandler::WindowOpenDisposition, bool,
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

int VirtualKey(const std::string& code, const std::string& key) {
  if (code.size() == 4 && code.rfind("Key", 0) == 0 && code[3] >= 'A' && code[3] <= 'Z') return code[3];
  if (code.size() == 6 && code.rfind("Digit", 0) == 0 && code[5] >= '0' && code[5] <= '9') return code[5];
  if (code.size() == 7 && code.rfind("Numpad", 0) == 0 && code[6] >= '0' && code[6] <= '9') return VK_NUMPAD0 + code[6] - '0';
  if (code.size() >= 2 && code.size() <= 3 && code[0] == 'F' && code[1] >= '1' && code[1] <= '9' &&
      (code.size() == 2 || (code[2] >= '0' && code[2] <= '9'))) {
    const int n = std::stoi(code.substr(1)); if (n <= 24) return VK_F1 + n - 1;
  }
  static const std::map<std::string, int> keys{
    {"Enter",VK_RETURN},{"NumpadEnter",VK_RETURN},{"Tab",VK_TAB},{"Space",VK_SPACE},
    {"Escape",VK_ESCAPE},{"Backspace",VK_BACK},{"Delete",VK_DELETE},{"Insert",VK_INSERT},
    {"Home",VK_HOME},{"End",VK_END},{"PageUp",VK_PRIOR},{"PageDown",VK_NEXT},
    {"ArrowLeft",VK_LEFT},{"ArrowRight",VK_RIGHT},{"ArrowUp",VK_UP},{"ArrowDown",VK_DOWN},
    {"ShiftLeft",VK_SHIFT},{"ShiftRight",VK_SHIFT},{"ControlLeft",VK_CONTROL},{"ControlRight",VK_CONTROL},
    {"AltLeft",VK_MENU},{"AltRight",VK_MENU},{"MetaLeft",VK_LWIN},{"MetaRight",VK_RWIN},
    {"CapsLock",VK_CAPITAL},{"NumLock",VK_NUMLOCK},{"ScrollLock",VK_SCROLL},
    {"Semicolon",VK_OEM_1},{"Equal",VK_OEM_PLUS},{"Comma",VK_OEM_COMMA},{"Minus",VK_OEM_MINUS},
    {"Period",VK_OEM_PERIOD},{"Slash",VK_OEM_2},{"Backquote",VK_OEM_3},
    {"BracketLeft",VK_OEM_4},{"Backslash",VK_OEM_5},{"BracketRight",VK_OEM_6},{"Quote",VK_OEM_7},
    {"IntlBackslash",VK_OEM_102},{"NumpadMultiply",VK_MULTIPLY},{"NumpadAdd",VK_ADD},
    {"NumpadSubtract",VK_SUBTRACT},{"NumpadDecimal",VK_DECIMAL},{"NumpadDivide",VK_DIVIDE},
  };
  auto found = keys.find(code);
  if (found != keys.end()) return found->second;
  found = keys.find(key);
  return found == keys.end() ? 0 : found->second;
}

void ApplyInput(CefRefPtr<View> view, CefRefPtr<CefDictionaryValue> command) {
  if (!view->browser || view->stopped || Number(command, "generation") != view->generation) return;
  auto host = view->browser->GetHost();
  const std::string type = command->GetString("type");
  if (type == "focus") {
    const bool focused = command->GetBool("focused");
    host->SetFocus(focused);
    if (!focused) {
      view->mouse_buttons = 0; view->input_modifiers = 0;
      host->SendCaptureLostEvent(); host->ImeCancelComposition();
    }
    return;
  }
  if (type == "modifiers") { view->input_modifiers = command->GetInt("modifiers") & 0x318f; return; }
  if (type == "key") {
    const uint32_t flags = command->GetInt("modifiers") & 0x3fff;
    view->input_modifiers = flags & 0x318f;
    const bool pressed = command->GetBool("pressed");
    const std::string code = command->GetString("code"), key = command->GetString("key");
    CefKeyEvent event;
    event.type = pressed ? KEYEVENT_RAWKEYDOWN : KEYEVENT_KEYUP;
    event.windows_key_code = VirtualKey(code, key);
    event.native_key_code = static_cast<int>(MapVirtualKeyW(event.windows_key_code, MAPVK_VK_TO_VSC) << 16);
    if (code == "ControlRight" || code == "AltRight" || code == "NumpadEnter" ||
        (event.windows_key_code >= VK_PRIOR && event.windows_key_code <= VK_DOWN) ||
        event.windows_key_code == VK_INSERT || event.windows_key_code == VK_DELETE)
      event.native_key_code |= 1 << 24;
    if (!pressed) event.native_key_code |= static_cast<int>(0xc0000000u);
    else if (flags & EVENTFLAG_IS_REPEAT) event.native_key_code |= 1 << 30;
    event.modifiers = flags;
    event.is_system_key = (flags & EVENTFLAG_ALT_DOWN) != 0;
    if (event.windows_key_code) host->SendKeyEvent(event);
    // Text travels separately from the physical key: preserve Unicode and
    // keyboard layout, while letting Chromium handle editing shortcuts.
    const bool shortcut = (flags & EVENTFLAG_COMMAND_DOWN) ||
      ((flags & EVENTFLAG_CONTROL_DOWN) && !(flags & EVENTFLAG_ALT_DOWN)) ||
      ((flags & EVENTFLAG_ALT_DOWN) && !(flags & EVENTFLAG_CONTROL_DOWN));
    if (pressed && !shortcut) {
      const std::u16string text = command->GetString("text").ToString16();
      for (const char16_t ch : text) {
        event.type = KEYEVENT_CHAR; event.windows_key_code = ch;
        event.character = ch; event.unmodified_character = ch;
        host->SendKeyEvent(event);
      }
    }
    return;
  }
  if (type == "ime_cancel") { host->ImeCancelComposition(); return; }
  if (type == "ime") {
    const CefString text = command->GetString("text");
    if (text.length() > 65536) return;
    if (command->GetBool("commit")) host->ImeCommitText(text, CefRange(UINT32_MAX, UINT32_MAX), 0);
    else {
      CefCompositionUnderline underline;
      underline.range = CefRange(0, static_cast<uint32_t>(text.length()));
      underline.color = CefColorSetARGB(255, 0, 0, 0);
      const auto end = static_cast<uint32_t>(text.length());
      host->ImeSetComposition(text, {underline}, CefRange(UINT32_MAX, UINT32_MAX), CefRange(end, end));
    }
    return;
  }
  auto coordinate = [&](const char* key, int maximum) {
    const double value = command->GetType(key) == VTYPE_DOUBLE ? command->GetDouble(key) : command->GetInt(key);
    return static_cast<int>(std::clamp(value, 0.0, static_cast<double>(maximum - 1)));
  };
  CefMouseEvent mouse;
  mouse.x = coordinate("x", view->width); mouse.y = coordinate("y", view->height);
  mouse.modifiers = view->input_modifiers | view->mouse_buttons;
  if (type == "move") host->SendMouseMoveEvent(mouse, false);
  else if (type == "button") {
    const int button = command->GetInt("button");
    if (button < 0 || button > 2) return;
    const bool pressed = command->GetBool("pressed");
    const uint32_t flag = button == 0 ? EVENTFLAG_LEFT_MOUSE_BUTTON :
      button == 1 ? EVENTFLAG_MIDDLE_MOUSE_BUTTON : EVENTFLAG_RIGHT_MOUSE_BUTTON;
    if (pressed) view->mouse_buttons |= flag; else view->mouse_buttons &= ~flag;
    mouse.modifiers = view->input_modifiers | view->mouse_buttons;
    if (pressed) {
      host->SetFocus(true);
      const auto now = GetTickCount64();
      if (view->last_button == button && now - view->last_click_time <= GetDoubleClickTime() &&
          abs(mouse.x - view->last_click_x) <= GetSystemMetrics(SM_CXDOUBLECLK) / 2 &&
          abs(mouse.y - view->last_click_y) <= GetSystemMetrics(SM_CYDOUBLECLK) / 2)
        view->click_count = view->click_count % 3 + 1;
      else view->click_count = 1;
      view->last_button = button; view->last_click_time = now;
      view->last_click_x = mouse.x; view->last_click_y = mouse.y;
    }
    host->SendMouseClickEvent(mouse, button == 0 ? MBT_LEFT : button == 1 ? MBT_MIDDLE : MBT_RIGHT,
      !pressed, view->click_count);
  } else if (type == "wheel") {
    auto delta = [&](const char* key) {
      const double value = command->GetType(key) == VTYPE_DOUBLE ? command->GetDouble(key) : command->GetInt(key);
      return static_cast<int>(std::clamp(value, -12000.0, 12000.0));
    };
    host->SendMouseWheelEvent(mouse, delta("dx"), delta("dy"));
  }
}
class ApplyCommand final : public CefTask {
 public:
  ApplyCommand(CefRefPtr<CefDictionaryValue> d, std::shared_ptr<std::promise<void>> done)
      : command_(d), done_(std::move(done)) {}
  void Execute() override {
    if (!context_ready) { CefPostDelayedTask(TID_UI, this, 10); return; }
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
    if (op == "input") {
      if (it != views.end()) ApplyInput(it->second, command_);
      return;
    }
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
class Application final : public CefApp, public CefBrowserProcessHandler {
 public:
  CefRefPtr<CefBrowserProcessHandler> GetBrowserProcessHandler() override { return this; }
  void OnContextInitialized() override {
    context_ready = true;
    if (credits_output.empty()) return;
    CefRefPtr<View> view = new View;
    view->url = "chrome://credits";
    views.emplace(0, view);
    CefWindowInfo window; window.SetAsWindowless(nullptr);
    CefBrowserSettings settings;
    if (!CefBrowserHost::CreateBrowser(window, view, "about:blank", settings, nullptr, nullptr)) {
      views.clear(); CefQuitMessageLoop();
    }
  }
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
  credits_output = command->GetSwitchValue("credits-output").ToWString();
  if (!CefInitialize(args, settings, app, sandbox_info)) return 3;
  if (!credits_output.empty()) {
    CefRunMessageLoop();
    CefShutdown();
    return credits_ok ? 0 : 4;
  }
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
