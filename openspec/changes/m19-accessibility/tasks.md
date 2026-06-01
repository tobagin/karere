## 1. GSettings schema

- [ ] 1.1 Add `reduce-motion` (b, default false) to the application gschema XML.
- [ ] 1.2 Add `focus-indicators-enhanced` (b, default false) to the application gschema XML.
- [ ] 1.3 Add `screen-reader-opts` (b, default false) to the application gschema XML.
- [ ] 1.4 Recompile the gschema in the build (verify `glib-compile-schemas` runs in meson/cargo build script).

## 2. Reduce-motion binding

- [ ] 2.1 In `src/window.rs`, read `reduce-motion` on construction and call `GtkSettings::default().set_property("gtk-enable-animations", !reduce)`.
- [ ] 2.2 Connect `settings.connect_changed(Some("reduce-motion"), ...)` to propagate runtime changes to `gtk-enable-animations`.
- [ ] 2.3 Manual verify: toggling at runtime causes AdwAnimation transitions and toast slide-ins to skip.

## 3. Enhanced focus indicators

- [ ] 3.1 Create `data/resources/style.css` with `.enhanced-focus *:focus { outline: 3px solid @accent_color; outline-offset: 2px; }` and analogous high-visibility rules for entries and list rows.
- [ ] 3.2 Register `style.css` in `data/resources/resources.gresource.xml`.
- [ ] 3.3 In `KarereApplication::startup`, instantiate a `gtk::CssProvider`, call `load_from_resource("/.../style.css")`, and add it to the default display at `STYLE_PROVIDER_PRIORITY_APPLICATION`.
- [ ] 3.4 In `src/window.rs`, bind `focus-indicators-enhanced` to toggle the `enhanced-focus` CSS class on the root window via `add_css_class` / `remove_css_class`.
- [ ] 3.5 Manual verify: with the setting `true`, focused buttons show 3 px accent-color rings.

## 4. Screen-reader caret-browsing flag

- [ ] 4.1 In `src/cef_runtime.rs::on_before_command_line_processing`, read the `screen-reader-opts` GSetting.
- [ ] 4.2 When `true`, append `--enable-caret-browsing` to the command line.
- [ ] 4.3 Manual verify: after restart, Chromium DevTools "Document settings" reports caret browsing active.

## 5. Documentation & integration

- [ ] 5.1 Note restart-required semantics for `screen-reader-opts` in code comments near the GSetting read site.
- [ ] 5.2 Coordinate with M22 to expose all three switches in the preferences page and surface the restart-required hint for `screen-reader-opts`.
