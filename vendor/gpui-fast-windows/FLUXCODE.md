# gpui-fast-windows 0.1.13, as FluxCode builds it

A copy of [gpui-fast-windows](https://github.com/longbridge/gpui-fast) 0.1.13
(Apache-2.0, see LICENSE) with three changes, in `src/directx_renderer.rs`,
`src/fast/frame.rs`, `src/window.rs` and `src/events.rs`. Search for "FluxCode" to find them.

1. **Path textures on demand.** Upstream makes two window-sized textures for drawing
   paths when the window opens, one of them 4x multisampled: five window-sized buffers,
   about 44 MB at 1920x1140. An editor's frames rarely draw paths (FluxCode: the install
   animation), so they are now made by the first frame that draws a path and released
   after 240 frames without one, or when the window is resized.
2. **Two swap-chain buffers** instead of three (the flip model's minimum): one
   window-sized buffer less. GPUI draws on demand, so the third bought nothing here.

3. **Acrylic that doesn't flicker while the window moves** (MuaraAI/FluxCode#94). The
   `Blurred` background used the accent policy's acrylic (`SetWindowCompositionAttribute`,
   state 4), which stutters and flickers while the window is dragged. From Windows 11
   22H2 (build 22621) the compositor's own acrylic is used instead
   (`DWMWA_SYSTEMBACKDROP_TYPE` = `DWMSBT_TRANSIENTWINDOW`); on older systems the move/size
   loop swaps acrylic for plain blur (state 3) until the move ends.

To update: copy the new upstream version over this folder and apply the three changes again.
