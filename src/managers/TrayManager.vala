/*
 * Copyright (C) 2025 Thiago Fernandes
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

namespace Karere {

    // Strict structs for SNI D-Bus Interface to ensure correct marshalling
    public struct SNIIcon {
        public int width;
        public int height;
        public uint8[] data;
    }

    public struct SNIToolTip {
        public string icon_name;
        public SNIIcon[] icon_data;
        public string title;
        public string description;
    }

    // Using org.kde.StatusNotifierItem for maximum compatibility 
    // (many implementations expect this legacy name despite the fdo spec)
    [DBus (name = "org.kde.StatusNotifierItem")]
    public interface StatusNotifierItem : Object {
        public abstract string category { owned get; }
        public abstract string id { owned get; }
        public abstract string title { owned get; }
        [DBus (name = "Status")]
        public abstract string item_status { owned get; }
        public abstract int window_id { owned get; }
        
        // Icon name (fallback)
        public abstract string icon_name { owned get; }
        
        // Icon data (primary for sandbox) - Using Variant with signature instead of struct
        [DBus (name = "IconPixmap", signature = "a(iiay)")]
        public abstract Variant icon_pixmap { owned get; }
        
        [DBus (name = "OverlayIconPixmap", signature = "a(iiay)")]
        public abstract Variant overlay_icon_pixmap { owned get; }

        [DBus (name = "AttentionIconPixmap", signature = "a(iiay)")]
        public abstract Variant attention_icon_pixmap { owned get; }
        
        public abstract string icon_theme_path { owned get; }
        public abstract string attention_icon_name { owned get; }
        public abstract string overlay_icon_name { owned get; }
        
        // ItemIsMenu is boolean
        public abstract bool item_is_menu { get; }
        
        // Menu is Object Path - Using Variant to avoid Vala/GObject string marshalling crashes
        [DBus (name = "Menu", signature = "o")]
        public abstract Variant menu { owned get; }
        
        // ToolTip - Using struct for strict marshalling
        [DBus (name = "ToolTip")]
        public abstract SNIToolTip tool_tip { owned get; }

        public abstract void context_menu(int x, int y);
        public abstract void activate(int x, int y);
        public abstract void secondary_activate(int x, int y);
        public abstract void scroll(int delta, string orientation);

        [DBus (name = "NewStatus")]
        public signal void new_status(string status);
        [DBus (name = "NewIcon")]
        public signal void new_icon();
        [DBus (name = "NewAttentionIcon")]
        public signal void new_attention_icon();
        [DBus (name = "NewOverlayIcon")]
        public signal void new_overlay_icon();
        [DBus (name = "NewToolTip")]
        public signal void new_tool_tip();
    }

    [DBus (name = "com.canonical.dbusmenu")]
    public interface DBusMenu : Object {
        public abstract uint version { get; }
        [DBus (name = "Status")]
        public abstract string menu_status { owned get; }
        public abstract void get_layout(int parentId, int recursionDepth, string[] propertyNames, out uint revision, out Variant layout);
        public abstract void get_group_properties(int[] ids, string[] propertyNames, out Variant properties);
        public abstract void get_property(int id, string name, out Variant value);
        public abstract void event(int id, string eventId, Variant data, uint timestamp);
        public abstract void about_to_show(int id, out bool needUpdate);
        
        [DBus (name = "LayoutUpdated")]
        public signal void layout_updated(uint revision, int parent);
        [DBus (name = "ItemActivationRequested")]
        public signal void item_activation_requested(int id, uint timestamp);
    }

    public class TrayManager : Object, StatusNotifierItem, DBusMenu {
        private const string SNI_WATCHER_NAME = "org.kde.StatusNotifierWatcher";
        private const string SNI_WATCHER_PATH = "/StatusNotifierWatcher";
        private const string SNI_WATCHER_IFACE = "org.kde.StatusNotifierWatcher";
        
        private DBusConnection? connection;
        private SettingsManager settings_manager;
        private string _status = "Active";
        private bool is_gnome = false;
        private bool has_unread = false;

        // DBusMenu state
        private uint menu_revision = 1;

        // Signals
        public signal void toggle_window();
        public signal void quit_application();

        // Implementation of StatusNotifierItem properties
        public string category { owned get { return "Communications"; } }
        public string id { owned get { return Config.APP_ID; } }
        public string title { owned get { return Config.APP_NAME; } }
        public string item_status { owned get { return has_unread ? "NeedsAttention" : "Active"; } }
        public int window_id { owned get { return 0; } }
        
        // Icon name - using the app's symbolic icon for tray
        public string icon_name { 
            owned get { 
                return Config.APP_ID + "-symbolic"; 
            } 
        }
        
        // Icon theme path - point to where our icons are installed
        public string icon_theme_path { owned get { return "/app/share/icons/hicolor"; } }
        
        // Cached icon data
        private SNIIcon? _cached_sni_icon = null;
        
        // Helper to load and rasterize the symbolic icon
        private SNIIcon? get_sni_icon_data() {
            if (_cached_sni_icon != null) {
                return _cached_sni_icon;
            }
            
            try {
                // Try to load SYMBOLIC SVG icon from installed path in sandbox
                string icon_path = "/app/share/icons/hicolor/symbolic/apps/" + Config.APP_ID + "-symbolic.svg";
                if (!FileUtils.test(icon_path, FileTest.EXISTS)) {
                    // Fallback path
                     icon_path = "/app/share/icons/hicolor/symbolic/apps/io.github.tobagin.karere-symbolic.svg";
                    if (!FileUtils.test(icon_path, FileTest.EXISTS)) {
                        warning("TrayManager: Symbolic Icon not found at expected paths");
                        return null;
                    }
                }
                
                // Load and rasterize SVG to a reasonable size (e.g. 24px)
                var pixbuf = new Gdk.Pixbuf.from_file_at_scale(icon_path, 24, 24, true);
                if (pixbuf == null) {
                    return null;
                }
                
                int width = pixbuf.width;
                int height = pixbuf.height;
                int n_channels = pixbuf.n_channels;
                int rowstride = pixbuf.rowstride;
                unowned uint8[] pixels = pixbuf.get_pixels();
                
                // Convert to ARGB format (network byte order)
                var argb_data = new uint8[width * height * 4];
                
                for (int y = 0; y < height; y++) {
                    for (int x = 0; x < width; x++) {
                        int src_idx = y * rowstride + x * n_channels;
                        int dst_idx = (y * width + x) * 4;
                        
                        uint8 r = pixels[src_idx];
                        uint8 g = pixels[src_idx + 1];
                        uint8 b = pixels[src_idx + 2];
                        uint8 a = (n_channels == 4) ? pixels[src_idx + 3] : 255;
                        
                        // ARGB format (network byte order: A, R, G, B)
                        argb_data[dst_idx] = a;
                        argb_data[dst_idx + 1] = r;
                        argb_data[dst_idx + 2] = g;
                        argb_data[dst_idx + 3] = b;
                    }
                }
                
                _cached_sni_icon = SNIIcon() {
                    width = width,
                    height = height,
                    data = argb_data
                };
                
                return _cached_sni_icon;
                
            } catch (Error e) {
                warning("TrayManager: Failed to load icon: %s", e.message);
                return null;
            }
        }

        // Helper to wrap SNIIcon into the Variant format expected by IconPixmap
        private Variant get_icon_variant() {
            var icon = get_sni_icon_data();
            if (icon == null) {
                return new Variant.array(new VariantType("(iiay)"), {});
            }
            
            SNIIcon valid_icon = (SNIIcon)icon;
            
            // Create the byte array variant
            var bytes = new GLib.Bytes(valid_icon.data);
            var byte_array = new Variant.from_bytes(new VariantType("ay"), bytes, true);
            
            // Create the icon struct: (iiay)
            var icon_struct = new Variant.tuple({
                new Variant.int32(valid_icon.width),
                new Variant.int32(valid_icon.height),
                byte_array
            });
            
            return new Variant.array(new VariantType("(iiay)"), { icon_struct });
        }

        // Primary Icon - Send rasterized symbolic icon for Flatpak visibility
        public Variant icon_pixmap { 
            owned get { 
                return get_icon_variant();
            } 
        }

        public string attention_icon_name { owned get { return Config.APP_ID + "-notification-symbolic"; } }
        public string overlay_icon_name { owned get { return ""; } }
        
        public Variant attention_icon_pixmap { 
           owned get { 
                return get_icon_variant();
           } 
        }
        
        public Variant overlay_icon_pixmap { 
           owned get { 
                return new Variant.array(new VariantType("(iiay)"), {});
           } 
        }

        // item_is_menu = false means left-click calls activate(), right-click shows menu
        public bool item_is_menu { get { return false; } } 
        
        // Menu - Return Object Path as Variant to avoid crash
        public Variant menu { 
            owned get { 
                return new Variant.object_path("/StatusNotifierMenu"); 
            } 
        }
        
        // ToolTip - Using struct for strict marshalling
        public SNIToolTip tool_tip { 
            owned get { 
                string desc = has_unread ? "Unread Messages" : "WhatsApp Client";
                var icon = get_sni_icon_data();
                SNIIcon[] icon_array = {};
                if (icon != null) {
                    icon_array += (SNIIcon)icon;
                }
                
                return SNIToolTip() {
                    icon_name = icon_name,
                    icon_data = icon_array,
                    title = Config.APP_NAME,
                    description = desc
                };
            } 
        }


        // Implementation of DBusMenu properties
        public uint version { get { return 3; } }
        public string menu_status { owned get { return "normal"; } }

        // Constructor - only needs settings logic
        public TrayManager() {
            settings_manager = SettingsManager.get_instance();
            // Environment check removed from constructor, moved to start/should_enable
            
            var desktop = Environment.get_variable("XDG_CURRENT_DESKTOP");
            if (desktop != null && (desktop.contains("GNOME") || desktop.contains("gnome"))) {
                is_gnome = true;
            }
        }

        private bool should_enable() {
            var force_tray = Environment.get_variable("KARERE_FORCE_TRAY") == "1";
            if (force_tray) return true;

            if (!settings_manager.is_initialized()) return false;
            var settings = settings_manager.get_settings();
            if (settings == null) return false;
            var mode = settings.get_int("system-tray-mode");
            if (mode == 2) return false;
            if (mode == 1) return true;
            return !is_gnome;
        }

        private void on_tray_mode_changed() {
            if (should_enable()) {
                 if (connection != null) {
                     register_objects();
                     register_with_watcher.begin(Config.APP_ID);
                 }
            } else {
                stop();
            }
        }
        
        public void set_unread_status(bool unread) {
            bool changed = (has_unread != unread);
            has_unread = unread;
            if (changed && connection != null) {
                // Icon cache removed 
                new_status(this.item_status);
                new_icon();
                new_attention_icon();
                new_tool_tip();
            }
        }

        // NEW START METHOD accepting external connection
        public void start(DBusConnection conn) {
            if (this.connection != null) return; // Already started
            
            if (!should_enable()) {
                message("TrayManager: Disabled by configuration.");
                return;
            }

            message("TrayManager: Starting with shared connection.");
            this.connection = conn;
            
            // Connect signal here now that we have context
            if (settings_manager.is_initialized()) {
                 var settings = settings_manager.get_settings();
                 if (settings != null) {
                     settings.changed["system-tray-mode"].connect(on_tray_mode_changed);
                 }
            }
            
            register_objects();
            register_with_watcher.begin(Config.APP_ID);
        }

        public void stop() {
            // No-op for now
        }

        private void register_objects() {
            if (connection == null) return;
            try {
                // Register objects on the shared connection
                connection.register_object("/StatusNotifierItem", (StatusNotifierItem)this);
                connection.register_object("/StatusNotifierMenu", (DBusMenu)this);
                debug("TrayManager: Registered /StatusNotifierItem and /StatusNotifierMenu");
            } catch (IOError e) {
                warning("TrayManager: Failed to register D-Bus objects: %s", e.message);
            }
        }
        
        private async void register_with_watcher(string service_name) {
            try {
                var watcher = yield DBusProxy.new (
                    connection,
                    DBusProxyFlags.DO_NOT_LOAD_PROPERTIES,
                    null,
                    SNI_WATCHER_NAME,
                    SNI_WATCHER_PATH,
                    SNI_WATCHER_IFACE,
                    null
                );
                yield watcher.call("RegisterStatusNotifierItem", 
                             new Variant.tuple({ new Variant.string(service_name) }),
                             DBusCallFlags.NONE, -1, null);
            } catch (Error e) {
            }
        }

        // --- StatusNotifierItem Methods ---

        public void context_menu(int x, int y) { }

        public void activate(int x, int y) {
            Idle.add(() => {
                toggle_window();
                return false;
            });
        }

        public void secondary_activate(int x, int y) {
            Idle.add(() => {
                toggle_window();
                return false;
            });
        }

        public void scroll(int delta, string orientation) {}

        // --- DBusMenu Implementation ---
        
        // Helper to get properties for a specific menu item ID
        private HashTable<string, Variant> get_menu_item_properties(int id) {
            var props = new HashTable<string, Variant>(str_hash, str_equal);
            
            switch (id) {
                case 0: // Root
                    props.insert("children-display", "submenu");
                    break;
                case 1: // Toggle
                    props.insert("label", "Show/Hide Karere");
                    props.insert("enabled", true);
                    props.insert("visible", true);
                    break;
                case 2: // Separator
                    props.insert("type", "separator");
                    props.insert("visible", true);
                    break;
                case 3: // Quit
                    props.insert("label", "Quit");
                    props.insert("enabled", true);
                    props.insert("visible", true);
                    break;
            }
            
            return props;
        }

        public void get_layout(int parentId, int recursionDepth, string[] propertyNames, out uint revision, out Variant layout) {
            revision = menu_revision;
            
            if (parentId == 0) {
                // Root children
                var empty_children = new Variant.array(new VariantType("v"), {});
                
                // Construct children manually using the helper for properties
                
                // Item 1: Toggle
                var item1_props = get_menu_item_properties(1);
                var item1 = new Variant.tuple({
                    new Variant.int32(1),
                    item1_props,
                    empty_children
                });
                
                // Item 2: Separator
                var item2_props = get_menu_item_properties(2);
                var item2 = new Variant.tuple({
                    new Variant.int32(2),
                    item2_props,
                    empty_children
                });
                
                // Item 3: Quit
                var item3_props = get_menu_item_properties(3);
                var item3 = new Variant.tuple({
                    new Variant.int32(3),
                    item3_props,
                    empty_children
                });
                
                var root_children = new Variant.array(new VariantType("v"), { 
                    new Variant.variant(item1),
                    new Variant.variant(item2),
                    new Variant.variant(item3)
                });
                
                var root_props = get_menu_item_properties(0);
                
                layout = new Variant.tuple({
                    new Variant.int32(0),
                    root_props,
                    root_children
                });
            } else {
                 var empty_children = new Variant.array(new VariantType("v"), {});
                 layout = new Variant.tuple({
                    new Variant.int32(parentId),
                    new HashTable<string, Variant>(str_hash, str_equal),
                    empty_children
                 });
            }
        }

        public void get_group_properties(int[] ids, string[] propertyNames, out Variant properties) {
             var builder = new VariantBuilder(new VariantType("a(ia{sv})"));
             
             foreach (int id in ids) {
                 var props = get_menu_item_properties(id);
                 builder.add("(ia{sv})", id, props);
             }
             
             properties = builder.end();
        }

        public new void get_property(int id, string name, out Variant value) {
             var props = get_menu_item_properties(id);
             if (props.contains(name)) {
                 value = props.get(name);
             } else {
                 value = new Variant.string("");
             }
        }

        public void event(int id, string eventId, Variant data, uint timestamp) {
            if (eventId == "clicked") {
                Idle.add(() => {
                    if (id == 1) { // Show/Hide
                        toggle_window();
                    } else if (id == 3) { // Quit
                        quit_application();
                    }
                    return false;
                });
            }
        }

        public void about_to_show(int id, out bool needUpdate) {
            needUpdate = false; 
        }
    }
}
