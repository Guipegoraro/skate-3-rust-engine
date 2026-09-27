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
3. Match: `python tools/audio_trace_match.py logs/xma-trace-<date>.log <game folder> --known` prints
   time, XMA context and `<bank>/<index>` per buffer (same ids as `audio.json`; ABK banks are
   resolved to their subsong by the sample count in `audio.json`).

## Without a player (scripted pad)

`scripts/Run-Skate3RecompTrace.ps1 -Automate` starts the game windowed with the recomp's own
demo path (`--skate3_demo_path=true` boots through the menus into the plaza, about 20 s) and
`--mnk_mode=true` (a virtual pad that answers even when the window is not focused). The patched
runtime then reads the pad from `logs/pad-script.txt` (`REX_PAD_SCRIPT`), replaying it from the
top whenever the file changes:

```
# ollie 1              marker: written into the trace as "# <ms> ollie 1" when reached
12 - 0 0 0 -1          <polls> <buttons|-> <lx> <ly> <rx> <ry> [lt] [rt]; ~60 polls per second
5 a+rb 0 0.5 0 1       buttons: a b x y start back lb rb l3 r3 up down left right
```

Copy a scenario from `scenarios/` (five pushes and ollies; walking plus pause-menu navigation;
five bails with the Xbox bail chord; five grind attempts on the handrail left of the spawn)
to `logs/pad-script.txt`, wait for its `end` marker in the trace, then summarise what follows
each action: `--recurring ollie 2.2` lists the ids that start within 2.2 s after most
`ollie <n>` markers. Pedestrians, other skaters and ambience play all the time, so only ids that
recur after every repetition count. Screenshots of the window: `PrintWindow` with flag 3
(`PW_RENDERFULLCONTENT`) captures the D3D12 frame without focusing the window.

The patch only adds logging when `REX_XMA_TRACE` is set, and the scripted pad only when
`REX_PAD_SCRIPT` is set. It also drops two `RasterizerGamma`
lines from the debug UI, a field that only exists in the fork's imgui.
