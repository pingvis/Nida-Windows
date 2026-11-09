"""Build the vendored AEC3 sources as one MSVC x64 static library.

Run using the local Meson/Ninja venv made by setup-windows-aec.ps1.
No source downloads or patches are needed; all three archives are vendored.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile


ARCHIVES = {
    "webrtc-audio-processing-2.1.tar.gz": "655ce719a9fa386df21ed9a9de02493f2157ede84100ed26c06c95d65631be60",
    "abseil-cpp-20240722.0.tar.gz": "f50e5ac311a81382da7fa75b97310e4b9006474f9560ac46f54a9967f07d4ae3",
    "abseil-cpp_20240722.0-3_patch.zip": "12dd8df1488a314c53e3751abd2750cf233b830651d168b6a9f15e7d0cf71f7b",
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if sys.platform != "win32" or os.environ.get("VSCMD_ARG_TGT_ARCH") != "x64":
        parser.error("Run from x64 MSVC Developer PowerShell")
    root = Path(__file__).resolve().parents[2]
    vendor = root / "vendor/webrtc-aec3"
    compiler = Path(shutil.which("cl.exe") or "missing-cl.exe")
    inputs = {name: digest(vendor / "upstream" / name) for name in ARCHIVES}
    if inputs != ARCHIVES:
        raise RuntimeError("Vendored AEC3 source checksum mismatch")
    inputs.update({
        "wrapper": digest(vendor / "src/kk_aec.cc"),
        "header": digest(vendor / "include/kk_aec.h"),
        "recipe": digest(Path(__file__)),
        "compiler": digest(compiler),
        "meson": "1.9.1", "ninja": "1.13.0", "crt": "MD", "arch": "x64",
    })
    output = args.output.resolve()
    # A changed recipe gets a fresh directory; never recursively delete a build.
    key = hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest()[:16]
    work = output / key
    marker = output / "nida-aec-build.json"
    library = output / "nida_aec.lib"
    if library.exists() and marker.exists():
        cached = json.loads(marker.read_text(encoding="utf-8"))
        if cached.get("inputs") == inputs and cached.get("library_sha256") == digest(library):
            print(f"AEC3 cache verified: {library}")
            return
    work.mkdir(parents=True, exist_ok=True)
    source = work / "webrtc-audio-processing-2.1"
    if not source.exists():
        with tarfile.open(vendor / "upstream/webrtc-audio-processing-2.1.tar.gz") as archive:
            archive.extractall(work, filter="data")
    # The upstream Meson recipe enables DLL exports even for static builds.
    # A static Nida link needs no WebRTC exports from the application itself.
    meson_file = source / "meson.build"
    meson_text = meson_file.read_text(encoding="utf-8")
    meson_file.write_text(meson_text.replace("  '-DWEBRTC_ENABLE_SYMBOL_EXPORT',\n", ""), encoding="utf-8")
    packagecache = source / "subprojects/packagecache"
    packagecache.mkdir(parents=True, exist_ok=True)
    for name in ARCHIVES:
        if name.startswith("abseil"):
            shutil.copyfile(vendor / "upstream" / name, packagecache / name)
    build = work / "build"
    if not (build / "build.ninja").exists():
        run(sys.executable, "-m", "mesonbuild.mesonmain", "setup", build, source,
            "--buildtype=release", "--default-library=static",
            "--force-fallback-for=abseil-cpp", "--wrap-mode=nodownload",
            "-Db_vscrt=md", "-Dcpp_std=c++20", "-Dabseil-cpp:cpp_std=c++20",
            "-Dwarning_level=0", "-Dabseil-cpp:warning_level=0")
    run(sys.executable, "-m", "mesonbuild.mesonmain", "compile", "-C", build,
        "-j", "4", "webrtc-audio-processing-2")
    wrapper = work / "kk_aec.obj"
    run(compiler, "/nologo", "/c", "/MD", "/EHsc", "/std:c++20", "/O2", "/W0",
        "/Zc:__cplusplus", "/DNOMINMAX", "/D_USE_MATH_DEFINES",
        "/DWEBRTC_LIBRARY_IMPL", "/D_WINSOCKAPI_",
        "/DNDEBUG", "/DWEBRTC_WIN", "/D_WIN32", "/DWEBRTC_ENABLE_AVX2",
        "/DWEBRTC_APM_DEBUG_DUMP=0", f"/I{source / 'webrtc'}",
        f"/I{source / 'subprojects/abseil-cpp-20240722.0'}",
        f"/I{vendor / 'include'}", f"/Fo{wrapper}", vendor / "src/kk_aec.cc")
    # Meson embeds its transitive static dependencies in this archive, including
    # runtime-dispatched AVX2 kernels and Abseil. Adding them again duplicates
    # objects. Despite its .a suffix this is an MSVC COFF archive.
    archive = build / "webrtc/modules/audio_processing/libwebrtc-audio-processing-2.a"
    if not archive.exists():
        raise RuntimeError("Meson produced no audio-processing static library")
    run("lib.exe", "/nologo", "/machine:x64", f"/out:{library}", wrapper, archive)
    marker.write_text(json.dumps({"inputs": inputs, "library_sha256": digest(library)}, indent=2)
                      + "\n", encoding="utf-8")
    print(f"AEC3 built: {library}")


if __name__ == "__main__":
    main()
