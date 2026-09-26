# Skate3Recomp XMA trace (SK-032)

Finds which original sample the real game plays for each event, by logging the XMA buffers the
Skate3Recomp runtime decodes and matching them byte for byte against the disc banks.

1. Runtime: clone `https://github.com/mchughalex/rexglue-skate3` at the commit pinned by your
   Skate3Recomp release (v2.0.2: `7eb0faf`) next to this repo as `../rexglue-audiotrace`, init
   submodules (imgui's pinned commit is gone upstream; its master works with this patch), apply
   `rexglue-xma-trace.patch`, then configure and build only `rexruntime` with LLVM clang + Ninja:
   `cmake -S ../rexglue-audiotrace -B ../rexglue-audiotrace-build -G Ninja -DCMAKE_BUILD_TYPE=Release
   -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ "-DCMAKE_C_FLAGS=-march=x86-64 -mtune=generic -mssse3"
   "-DCMAKE_CXX_FLAGS=-march=x86-64 -mtune=generic -mssse3" -DCMAKE_CXX_STANDARD=23 -DREXGLUE_USE_VULKAN=ON`
   and `ninja -C ../rexglue-audiotrace-build rexruntime` (output: `../rexglue-audiotrace/out/win-amd64/rexruntime.dll`).
2. Run: `scripts/Run-Skate3RecompTrace.ps1` copies `skate3.exe` + the traced DLL to `../s3r-trace-run`
   (game/dlc as junctions, the install is untouched) and sets `REX_XMA_TRACE`.
3. Match: `python tools/audio_trace_match.py logs/xma-trace-<date>.log <game folder>` prints
   time, XMA context and `<bank>/<index>` per buffer (same ids as `audio.json`).

The patch only adds logging when `REX_XMA_TRACE` is set. It also drops two `RasterizerGamma`
lines from the debug UI, a field that only exists in the fork's imgui.
