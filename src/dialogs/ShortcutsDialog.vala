/*
 * Copyright (C) 2025 Thiago Fernandes
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 */

namespace Karere {

    /**
     * Keyboard shortcuts help dialog using AdwShortcutsDialog
     * This class loads and returns an AdwShortcutsDialog from the UI resource.
     *
     * Note: We use a factory pattern here because AdwShortcutsDialog must be
     * loaded from a UI file and cannot be easily subclassed with content.
     */
    public class ShortcutsDialog : GLib.Object {
        private Adw.ShortcutsDialog dialog;
        private GLib.Settings settings;

        public ShortcutsDialog(Gtk.Window parent) {
            settings = new GLib.Settings(Config.APP_ID);
            dialog = load_shortcuts_dialog();
            debug("ShortcutsDialog created with AdwShortcutsDialog");
        }

        /**
         * Load the shortcuts dialog from UI resource
         */
        private Adw.ShortcutsDialog load_shortcuts_dialog() {
            try {
                var builder = new Gtk.Builder();
                builder.add_from_resource("/" + Config.APP_ID.replace(".", "/") + "/shortcuts-dialog.ui");

                var shortcuts_dialog = builder.get_object("shortcuts_dialog") as Adw.ShortcutsDialog;

                if (shortcuts_dialog != null) {
                    debug("Shortcuts dialog loaded successfully");
                    return shortcuts_dialog;
                } else {
                    critical("Failed to load shortcuts_dialog from UI resource");
                    // Return a fallback empty dialog
                    return new Adw.ShortcutsDialog();
                }
            } catch (Error e) {
                critical("Error loading shortcuts dialog UI: %s", e.message);
                // Return a fallback empty dialog
                return new Adw.ShortcutsDialog();
            }
        }

        /**
         * Present the shortcuts dialog
         */
        public void present(Gtk.Window parent) {
            dialog.present(parent);
        }
    }
}
