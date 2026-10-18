# Capture-friendly native materials

Goal: Glass/Liquid remain visible in Windows screenshots without reading their
own foreground into a background effect. Implementation and commit/push authorized.

Evidence: the live widget's display affinity was 17, excluding the whole HWND.
GDI without exclusion reads its own foreground. A private Magnifier prototype was
rejected after reproducible multi-monitor crashes; it is not part of the product.

Use Windows Composition HostBackdropBrush plus Gaussian blur behind the existing
HWND. Clip native surfaces to the existing notch/detail geometry. Keep foreground,
placement, hit testing and all three persisted themes. Glass's CPU refraction lens
is replaced by native blur; preserve neutral tint and mirrored rim.

Verification gates:
- Ordered geometry updates clear on fold/hide/theme exit/reload, reject malformed
  resources, and remain idle without pixel polling or IPC.
- Actual Windows screenshots include foreground with material ready and affinity 0;
  moving between physical monitors and changing backdrops do not crash or recurse.
- Native detail/drag/settings geometry remains unchanged.
- npm test, npm run build, Clippy and browser lifecycle fixtures pass before push.

References:
- https://learn.microsoft.com/en-us/uwp/api/windows.ui.composition.compositor.createhostbackdropbrush
- https://learn.microsoft.com/en-us/windows/uwp/composition/using-the-visual-layer-with-win32
