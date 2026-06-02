## 1. GSettings schema

- [x] 1.1 Add `reduce-motion` (b, default false) to the application gschema XML.
- [x] 1.2 Add `focus-indicators` (b, default false) to the application gschema XML. (Shipped key is `focus-indicators`, not `focus-indicators-enhanced`; matches the M22 preferences binding.)
- [x] 1.3 Add `screen-reader-opts` (b, default false) to the application gschema XML.
- [x] 1.4 Recompile the gschema in the build (verify `glib-compile-schemas` runs in meson/cargo build script). (`meson.build` sets `gnome` `glib_compile_schemas: true`; `data/meson.build` substitutes + installs the schema.)

## 2. Reduce-motion binding

- [x] 2.1 In `src/window.rs`, read `reduce-motion` on construction and call `GtkSettings::default().set_property("gtk-enable-animations", !reduce)`.
- [x] 2.2 Connect `settings.connect_changed(Some("reduce-motion"), ...)` to propagate runtime changes to `gtk-enable-animations`.
- [x] 2.3 Manual verify: toggling at runtime causes AdwAnimation transitions and toast slide-ins to skip.

## 3. Enhanced focus indicators

- [x] 3.1 Create `data/style.css` with `.enhanced-focus *:focus { outline: 3px solid @accent_color; outline-offset: 2px; }` and analogous high-visibility rules for entries and list rows. (Project layout uses `data/`, not `data/resources/`.)
- [x] 3.2 Register `style.css` in `data/karere.gresource.xml`. (Project uses `karere.gresource.xml`, not `resources.gresource.xml`.)
- [x] 3.3 In `KarereApplication::startup`, instantiate a `gtk::CssProvider`, call `load_from_resource("/.../style.css")`, and add it to the default display at `STYLE_PROVIDER_PRIORITY_APPLICATION`.
- [x] 3.4 In `src/window.rs`, bind `focus-indicators` to toggle the `enhanced-focus` CSS class on the root window via `add_css_class` / `remove_css_class`.
- [x] 3.5 Manual verify: with the setting `true`, focused buttons show 3 px accent-color rings.

## 4. Screen-reader caret-browsing flag

- [x] 4.1 In `src/cef_runtime.rs::on_before_command_line_processing`, read the `screen-reader-opts` GSetting.
- [x] 4.2 When `true`, append `--enable-caret-browsing` to the command line.
- [x] 4.3 Manual verify: after restart, Chromium DevTools "Document settings" reports caret browsing active.

## 5. Documentation & integration

- [x] 5.1 Note restart-required semantics for `screen-reader-opts` in code comments near the GSetting read site.
- [x] 5.2 Coordinate with M22 to expose all three switches in the preferences page and surface the restart-required hint for `screen-reader-opts`. (Delivered by `m22-preferences-shortcuts-dialog`: the Accessibility page binds `reduce-motion`, `focus-indicators`, and `screen-reader-opts`, the latter with the restart-required subtitle.)
