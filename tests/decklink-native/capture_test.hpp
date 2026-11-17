// Behavioral capture coverage using the real shared shim. Included after the
// common mode/iterator doubles in shim_test.cpp; native ABI is checked separately.
#pragma once

struct InputDouble : IDeckLinkInput {
    IteratorDouble iterator;
    IDeckLinkInputCallback* callback = nullptr;
    int refs = 0, starts = 0, stops = 0, pauses = 0, flushes = 0;
    bool fail_enable = false, fail_start = false, fail_audio = false;
    bool video_enabled = false, audio_enabled = false;
    BMDPixelFormat format = bmdFormat8BitYUV;
    BMDVideoInputFlags flags = 0;
    HRESULT DoesSupportVideoMode(int, BMDDisplayMode mode, BMDPixelFormat, int, int,
                                BMDDisplayMode*, DlSupported* supported) override {
        *supported = mode == 1; return S_OK;
    }
    HRESULT GetDisplayModeIterator(IDeckLinkDisplayModeIterator** out) override {
        iterator.next = 0; ++iterator.refs; *out = &iterator; return S_OK;
    }
    HRESULT GetDisplayMode(BMDDisplayMode code, IDeckLinkDisplayMode** out) override {
        if (code != 1) { *out = nullptr; return E_FAIL; }
        iterator.mode.code = code;
        ++iterator.mode.refs; *out = &iterator.mode; return S_OK;
    }
    HRESULT SetCallback(IDeckLinkInputCallback* next) override {
        if (next) next->AddRef();
        if (callback) callback->Release();
        callback = next; return S_OK;
    }
    HRESULT EnableVideoInput(BMDDisplayMode, BMDPixelFormat pixels, BMDVideoInputFlags options) override {
        if (fail_enable) return E_FAIL;
        video_enabled = true; format = pixels; flags = options; return S_OK;
    }
    HRESULT EnableAudioInput(int rate, int type, unsigned int channels) override {
        assert(rate == 48000 && type == 32 && channels == 2);
        audio_enabled = !fail_audio; return fail_audio ? E_FAIL : S_OK;
    }
    HRESULT DisableVideoInput() override { video_enabled = false; return S_OK; }
    HRESULT DisableAudioInput() override { audio_enabled = false; return S_OK; }
    HRESULT StartStreams() override { ++starts; return fail_start ? E_FAIL : S_OK; }
    HRESULT StopStreams() override { ++stops; return S_OK; }
    HRESULT PauseStreams() override { ++pauses; return S_OK; }
    HRESULT FlushStreams() override { ++flushes; return S_OK; }
    ULONG Release() override { assert(refs > 0); return --refs; }
};

struct AttributesDouble : IDeckLinkProfileAttributes {
    int refs = 0;
    bool detects = true;
    HRESULT GetFlag(int key, DlSupported* value) override {
        assert(key == BMDDeckLinkSupportsInputFormatDetection);
        *value = detects; return S_OK;
    }
    HRESULT GetInt(int key, int64_t* value) override {
        *value = key == BMDDeckLinkPersistentID ? 0x1234 : 3; return S_OK;
    }
    ULONG Release() override { assert(refs > 0); return --refs; }
};

struct ConfigurationDouble : IDeckLinkConfiguration {
    int refs = 0;
    int64_t connection = 1;
    HRESULT SetInt(int key, int64_t value) override {
        assert(key == bmdDeckLinkConfigVideoInputConnection);
        connection = value; return S_OK;
    }
    HRESULT GetInt(int key, int64_t* value) override {
        assert(key == bmdDeckLinkConfigVideoInputConnection);
        *value = connection; return S_OK;
    }
    ULONG Release() override { assert(refs > 0); return --refs; }
};

struct CaptureDeviceDouble : IDeckLink {
    InputDouble input;
    AttributesDouble attributes;
    ConfigurationDouble config;
    int refs = 0;
    HRESULT GetDisplayName(DlString* name) override { *name = "DeckLink Input"; return S_OK; }
    HRESULT QueryInterface(REFIID iid, LPVOID* out) override {
        *out = nullptr;
        if (iid == IID_IDeckLinkInput) { ++input.refs; *out = &input; }
        else if (iid == IID_IDeckLinkProfileAttributes) { ++attributes.refs; *out = &attributes; }
        else if (iid == IID_IDeckLinkConfiguration) { ++config.refs; *out = &config; }
        return *out ? S_OK : E_NOINTERFACE;
    }
    ULONG AddRef() override { return ++refs; }
    ULONG Release() override { assert(refs > 0); return --refs; }
    void released() const {
        assert(refs == 0 && input.refs == 0 && attributes.refs == 0 && config.refs == 0);
        assert(input.callback == nullptr && !input.video_enabled && !input.audio_enabled);
        assert(input.iterator.refs == 0 && input.iterator.mode.refs == 0);
    }
};

struct InputFrameDouble : IDeckLinkVideoInputFrame {
    uint8_t bytes[24]{};
    bool no_signal = false;
    HRESULT GetBytes(void** out) override { *out = bytes; return S_OK; }
    long GetRowBytes() override { return 12; }
    long GetWidth() override { return 4; }
    long GetHeight() override { return 2; }
    BMDPixelFormat GetPixelFormat() override { return bmdFormat8BitYUV; }
    uint32_t GetFlags() override { return no_signal ? bmdFrameHasNoInputSource : 0; }
    ULONG Release() override { return 1; }
};

struct InputAudioDouble : IDeckLinkAudioInputPacket {
    int32_t samples[4] = {100, -100, 200, -200};
    long GetSampleFrameCount() override { return 2; }
    HRESULT GetBytes(void** out) override { *out = samples; return S_OK; }
};

struct CapturedDouble {
    int pictures = 0, sounds = 0;
    bool no_signal = false;
    static void video(void* ctx, const uint8_t* data, int32_t row, int32_t width, int32_t height,
                      uint32_t pixels, uint32_t, int32_t no_signal, int32_t progressive,
                      int64_t duration, int64_t scale) {
        auto& taken = *static_cast<CapturedDouble*>(ctx);
        assert(data && row == 12 && width == 4 && height == 2);
        assert(pixels == bmdFormat8BitYUV && progressive == 1);
        assert(duration == 1001 && scale == 60000);
        ++taken.pictures; taken.no_signal = no_signal != 0;
    }
    static void audio(void* ctx, const int32_t* samples, int32_t frames, int32_t channels) {
        auto& taken = *static_cast<CapturedDouble*>(ctx);
        assert(frames == 2 && channels == 2);
        assert(samples[0] == 100 && samples[1] == -100 && samples[2] == 200 && samples[3] == -200);
        ++taken.sounds;
    }
};

void capture_lifecycle() {
    CaptureDeviceDouble device;
    device.input.iterator.mode.duration = 1001;
    device.input.iterator.mode.scale = 60000;
    const auto generation = nida_dl_generation();
    g_notifications.DeckLinkDeviceArrived(&device);
    assert(nida_dl_generation() == generation + 1);
    nida_dl_input_info infos[4]{};
    assert(nida_dl_inputs(infos, 4) == 1);
    const auto id = infos[0].id;
    assert(infos[0].persistent == 0x1234 && infos[0].connections == 3 && infos[0].detects == 1);
    assert(std::string(infos[0].name) == "DeckLink Input");
    nida_dl_in_mode modes[4]{};
    assert(nida_dl_input_modes(id, modes, 4) == 1 && modes[0].code == 1);
    assert(modes[0].frame_duration == 1001 && modes[0].time_scale == 60000 && modes[0].progressive == 1);
    char error[256]{};
    CapturedDouble taken;
    auto open = [&](uint64_t arrival = 0, uint32_t code = 1) {
        return nida_dl_capture_open(arrival ? arrival : id, code, &taken,
                                    CapturedDouble::video, CapturedDouble::audio, error, sizeof(error));
    };
    assert(open(id, 999) == nullptr);
    fake_ready = false;
    assert(open() == nullptr && std::string(error).find("COM") != std::string::npos);
    fake_ready = true;
    device.input.fail_enable = true;
    assert(open() == nullptr && device.input.callback == nullptr && device.input.refs == 0 && device.config.refs == 0);
    device.input.fail_enable = false;
    device.input.fail_start = true;
    assert(open() == nullptr && device.input.callback == nullptr && !device.input.video_enabled && !device.input.audio_enabled);
    device.input.fail_start = false;
    device.input.fail_audio = true; // Picture capture is allowed without embedded audio.
    void* handle = open();
    assert(handle && nida_dl_capture_alive(handle) == 1 && !device.input.audio_enabled);
    assert(device.input.flags == bmdVideoInputEnableFormatDetection);
    assert(nida_dl_capture_connection(handle) == 1);
    assert(nida_dl_capture_set_connection(handle, 2) == 0 && nida_dl_capture_connection(handle) == 2);
    InputFrameDouble frame;
    InputAudioDouble audio;
    auto* callback = device.input.callback;
    void* queried = nullptr;
    assert(callback->QueryInterface(0, nullptr) == E_POINTER);
    assert(callback->QueryInterface(IID_IDeckLinkInputCallback, &queried) == S_OK && queried == callback);
    static_cast<IDeckLinkInputCallback*>(queried)->Release();
    assert(callback->QueryInterface(999, &queried) == E_NOINTERFACE && queried == nullptr);
    callback->VideoInputFrameArrived(&frame, &audio);
    assert(taken.pictures == 1 && taken.sounds == 1 && !taken.no_signal);
    frame.no_signal = true;
    callback->VideoInputFrameArrived(&frame, nullptr);
    assert(taken.no_signal);
    callback->VideoInputFormatChanged(bmdVideoInputDisplayModeChanged, &device.input.iterator.mode, bmdDetectedVideoInputRGB444);
    assert(device.input.format == bmdFormat8BitBGRA && device.input.pauses == 1 && device.input.flushes == 1);
    assert(nida_dl_capture_detected(handle) == 1);
    device.input.fail_enable = true;
    callback->VideoInputFormatChanged(bmdVideoInputDisplayModeChanged, &device.input.iterator.mode, 0);
    assert(nida_dl_capture_alive(handle) == 0);
    device.input.fail_enable = false;
    // A driver may retain a callback after unregistering. No more Rust context
    // access or stream restarts are permitted once capture close returns.
    callback->AddRef();
    nida_dl_capture_close(handle);
    const int starts = device.input.starts;
    callback->VideoInputFrameArrived(&frame, &audio);
    callback->VideoInputFormatChanged(bmdVideoInputDisplayModeChanged, &device.input.iterator.mode, 0);
    assert(taken.pictures == 2 && taken.sounds == 1 && device.input.starts == starts);
    callback->Release();
    handle = open();
    assert(handle);
    g_notifications.DeckLinkDeviceRemoved(&device);
    assert(nida_dl_capture_alive(handle) == 0 && nida_dl_inputs(infos, 4) == 0);
    assert(open() == nullptr);
    nida_dl_capture_close(handle);
    device.released();
    device.attributes.detects = false;
    g_notifications.DeckLinkDeviceArrived(&device);
    assert(nida_dl_inputs(infos, 4) == 1 && infos[0].id != id && !infos[0].detects);
    assert(open() == nullptr); // Stale arrival cannot open a reconnected device.
    handle = open(infos[0].id);
    assert(handle && device.input.flags == bmdVideoInputFlagDefault);
    callback = device.input.callback;
    callback->AddRef();
    nida_dl_shutdown();
    nida_dl_shutdown();
    callback->VideoInputFrameArrived(&frame, &audio);
    assert(taken.pictures == 2 && taken.sounds == 1 && device.input.callback == nullptr);
    nida_dl_capture_close(handle);
    callback->Release();
    assert(open(infos[0].id) == nullptr); // No capture may start after shutdown.
    g_notifications.DeckLinkDeviceRemoved(&device);
    device.released();
}
