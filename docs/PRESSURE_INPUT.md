# Pressure input support (M1.1, #15)

This is a backend capability investigation and synthetic test result, not physical
hardware certification. No physical pen hardware was tested.

| Runtime | Pressure path | Limitation/fallback |
| --- | --- | --- |
| Browser | Primary PointerEvent with pointerType `pen`, pressure → Rust raw-input hook | Requires browser/OS/device to report force. Mouse/finger keep eframe handling. Constant pressure reports remain constant. |
| Windows desktop | winit 0.30.13 WM_POINTER PT_PEN → egui-winit 0.37 Touch.force | Source-verified path, not hardware-tested. Legacy WM_TOUCH or mouse-only tablet drivers lack force. |
| Linux X11/Wayland desktop | Existing pointer drawing | winit touch constructors supply force=None; no tablet protocol/valuator adapter added. Constant-width fallback. |
| macOS desktop | Existing pointer drawing | winit's TouchpadPressure is a trackpad gesture, ignored by egui-winit; it is not stylus force. No NSEvent tablet adapter. Constant-width fallback. |

Sources inspected in the pinned local registry:
- `egui-winit-0.37.0/src/lib.rs`: on_touch normalizes Force and emits Touch before
  primary mouse emulation; TouchpadPressure remains unhandled.
- `winit-0.30.13/src/platform_impl/windows/event_loop.rs`:
  normalize_pointer_pressure, WM_POINTER PT_PEN/GetPointerPenInfo; WM_TOUCH has no force.
- `winit-0.30.13/src/platform_impl/linux/{x11/event_processor.rs,wayland/seat/touch/mod.rs}`:
  no force in current contact paths.
- `winit-0.30.13/src/platform_impl/macos/view.rs`: pressureChangeWithEvent emits
  TouchpadPressure, with no general tablet pressure delivery through this backend.
- `eframe-0.37.0/src/web/events.rs`: mouse and TouchEvent handling; this change
  adds a pen-only PointerEvent bridge rather than changing generic finger input.

Browser API references:
[PointerEvent pressure](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent/pressure),
[pointerType](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent/pointerType),
[Pointer Events specification](https://www.w3.org/TR/pointerevents/).
The API can return a default 0.5 for pressure-incapable devices and provides no
reliable separate pressure-capability flag. XiToc forwards reported pen values;
it does not label inferred variation or mouse defaults as hardware measurements.
Native `None` and missing/invalid samples explicitly mean unavailable.

A cross-platform native tablet adapter would require backend/OS work and matching
hardware verification. No major dependencies were added. M2 must persist raw
pressure, smoothing/pressure response metadata and matching visible PDF vectors.
Handwriting is still memory-only, so restarting discards ink; preferences persist.
