// Quiet, deterministic WASAPI test input through Windows' ordinary waveOut
// path. Build in a VS Developer shell: cl /EHsc /O2 windows-app-audio-tone.cpp
// /link winmm.lib. Requires an explicit, approved non-speaker device ID:
// --device <waveOut device index> <frequency Hz> <duration seconds>.
// It never falls back to the computer's default output.
#include <windows.h>
#include <mmsystem.h>
#include <cmath>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <vector>

int main(int argc, char** argv) {
    constexpr unsigned rate = 48000;
    if (argc != 5 || std::strcmp(argv[1], "--device") != 0) {
        std::cerr << "An explicitly approved non-speaker waveOut device is required. No playback started.\n";
        return 2;
    }
    char* deviceEnd = nullptr;
    const unsigned deviceIndex = std::strtoul(argv[2], &deviceEnd, 10);
    if (argv[2][0] == '\0' || argv[2][0] == '-' || *deviceEnd != '\0') return 2;
    const double hz = std::strtod(argv[3], nullptr);
    const unsigned seconds = std::strtoul(argv[4], nullptr, 10);
    if (!(hz >= 20.0 && hz <= 20000.0) || seconds == 0 || seconds > 3600) return 2;
    WAVEFORMATEX format{};
    format.wFormatTag = WAVE_FORMAT_PCM;
    format.nChannels = 2;
    format.nSamplesPerSec = rate;
    format.wBitsPerSample = 16;
    format.nBlockAlign = 4;
    format.nAvgBytesPerSec = rate * 4;
    HWAVEOUT device = nullptr;
    if (deviceIndex >= waveOutGetNumDevs()) return 3;
    if (waveOutOpen(&device, deviceIndex, &format, 0, 0, CALLBACK_NULL) != MMSYSERR_NOERROR) return 3;
    std::vector<short> pcm(rate * 2);
    for (unsigned frame = 0; frame < rate; ++frame) {
        const short sample = static_cast<short>(160.0 * std::sin(6.283185307179586 * hz * frame / rate));
        pcm[frame * 2] = pcm[frame * 2 + 1] = sample;
    }
    WAVEHDR header{};
    header.lpData = reinterpret_cast<LPSTR>(pcm.data());
    header.dwBufferLength = static_cast<DWORD>(pcm.size() * sizeof(short));
    header.dwFlags = WHDR_BEGINLOOP | WHDR_ENDLOOP;
    header.dwLoops = seconds;
    if (waveOutPrepareHeader(device, &header, sizeof(header)) != MMSYSERR_NOERROR) { waveOutClose(device); return 4; }
    if (waveOutWrite(device, &header, sizeof(header)) != MMSYSERR_NOERROR) { waveOutUnprepareHeader(device, &header, sizeof(header)); waveOutClose(device); return 5; }
    std::cout << "pid=" << GetCurrentProcessId() << " frequency=" << hz << std::endl;
    while (!(header.dwFlags & WHDR_DONE)) Sleep(20);
    waveOutUnprepareHeader(device, &header, sizeof(header));
    waveOutClose(device);
}
