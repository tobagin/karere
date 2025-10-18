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
     * ClipboardManager handles clipboard image paste operations.
     *
     * This manager detects Ctrl+V keypresses, checks for clipboard images,
     * and injects them into WhatsApp Web via JavaScript.
     */
    public class ClipboardManager : Object {

        // Signals
        public signal void paste_started();
        public signal void paste_succeeded(string image_type);
        public signal void paste_failed(string error_message);

        private Gdk.Clipboard clipboard;
        private WebKit.WebView web_view;

        /**
         * Create a new ClipboardManager
         *
         * @param clipboard The clipboard to monitor
         * @param web_view The WebView to inject images into
         */
        public ClipboardManager(Gdk.Clipboard clipboard, WebKit.WebView web_view) {
            this.clipboard = clipboard;
            this.web_view = web_view;
        }

        /**
         * Setup paste detection on the WebView
         *
         * @param widget The widget to add the event controller to
         */
        public void setup_paste_detection(Gtk.Widget widget) {
            // Set up key event controller to detect Ctrl+V
            var key_controller = new Gtk.EventControllerKey();
            key_controller.key_pressed.connect(on_key_pressed);

            // Add the controller to the widget
            widget.add_controller(key_controller);

            debug("Clipboard paste detection configured");
        }

        /**
         * Handle key press events to detect Ctrl+V for image paste
         */
        private bool on_key_pressed(uint keyval, uint keycode, Gdk.ModifierType state) {
            // Check for Ctrl+V (paste)
            if (keyval == Gdk.Key.v && (state & Gdk.ModifierType.CONTROL_MASK) != 0) {
                debug("Ctrl+V detected, checking clipboard for image");
                // Check synchronously if we have an image
                if (has_image_in_clipboard()) {
                    handle_paste_request();
                    return true; // Consume the event only if we have an image
                } else {
                    debug("No image in clipboard, allowing default paste");
                    return false; // Let default paste behavior happen
                }
            }

            return false; // Let other key events pass through
        }

        /**
         * Check if clipboard contains an image (synchronous check)
         */
        private bool has_image_in_clipboard() {
            if (clipboard == null) {
                return false;
            }

            var formats = clipboard.get_formats();
            return formats.contain_gtype(typeof(Gdk.Texture)) ||
                   formats.contain_mime_type("image/png") ||
                   formats.contain_mime_type("image/jpeg") ||
                   formats.contain_mime_type("image/gif");
        }

        /**
         * Handle a paste request from the user
         */
        public void handle_paste_request() {
            if (clipboard == null) {
                warning("Clipboard not available");
                return;
            }

            // Check if clipboard contains an image
            var formats = clipboard.get_formats();

            if (formats.contain_gtype(typeof(Gdk.Texture))) {
                // Clipboard contains an image texture
                paste_started();
                clipboard.read_texture_async.begin(null, (obj, res) => {
                    try {
                        var texture = clipboard.read_texture_async.end(res);
                        if (texture != null) {
                            process_clipboard_image(texture);
                        }
                    } catch (Error e) {
                        critical("Failed to read texture from clipboard: %s", e.message);
                        paste_failed(_("Failed to read clipboard image"));
                    }
                });
            } else if (formats.contain_mime_type("image/png") ||
                       formats.contain_mime_type("image/jpeg") ||
                       formats.contain_mime_type("image/gif")) {
                // Clipboard contains image data
                paste_started();
                string[] mime_types = {"image/png", "image/jpeg", "image/gif"};
                clipboard.read_async.begin(mime_types, GLib.Priority.DEFAULT, null, (obj, res) => {
                    try {
                        string mime_type;
                        var input_stream = clipboard.read_async.end(res, out mime_type);
                        if (input_stream != null) {
                            process_clipboard_image_stream(input_stream, mime_type);
                        }
                    } catch (Error e) {
                        critical("Failed to read image stream from clipboard: %s", e.message);
                        paste_failed(_("Failed to read clipboard image"));
                    }
                });
            } else {
                debug("No image found in clipboard, this shouldn't be reached");
                // This code path shouldn't be reached since we check has_image_in_clipboard() first
            }
        }

        /**
         * Process a clipboard image texture
         */
        private void process_clipboard_image(Gdk.Texture texture) {
            info("Processing clipboard image texture: %dx%d", texture.get_width(), texture.get_height());

            try {
                // Convert texture to PNG bytes
                var bytes = texture.save_to_png_bytes();
                inject_image_into_whatsapp(bytes, "image/png");
            } catch (Error e) {
                critical("Failed to convert texture to PNG: %s", e.message);
                paste_failed(_("Failed to process clipboard image"));
            }
        }

        /**
         * Process a clipboard image stream
         */
        private void process_clipboard_image_stream(GLib.InputStream input_stream, string mime_type) {
            info("Processing clipboard image stream with MIME type: %s", mime_type);

            try {
                // Read the entire stream into memory
                var output_stream = new MemoryOutputStream(null, GLib.realloc, GLib.free);
                output_stream.splice(input_stream, OutputStreamSpliceFlags.CLOSE_SOURCE | OutputStreamSpliceFlags.CLOSE_TARGET);

                var data = output_stream.steal_data();
                var bytes = new Bytes(data);

                inject_image_into_whatsapp(bytes, mime_type);
            } catch (Error e) {
                critical("Failed to process clipboard image stream: %s", e.message);
                paste_failed(_("Failed to process clipboard image"));
            }
        }

        /**
         * Inject image data into WhatsApp Web
         */
        private void inject_image_into_whatsapp(Bytes image_bytes, string mime_type) {
            info("Injecting %s image (%zu bytes) into WhatsApp Web", mime_type, image_bytes.get_size());

            // Convert bytes to base64 data URL
            var base64_data = Base64.encode(image_bytes.get_data());
            var data_url = "data:%s;base64,%s".printf(mime_type, base64_data);

            // JavaScript to inject the image into WhatsApp Web
            var javascript = """
                (function() {
                    try {
                        // Find the message input area
                        const messageBox = document.querySelector('[contenteditable="true"][data-tab="10"]') ||
                                          document.querySelector('[contenteditable="true"]') ||
                                          document.querySelector('div[contenteditable="true"]');

                        if (!messageBox) {
                            console.log('WhatsApp message box not found');
                            return;
                        }

                        // Focus the message box
                        messageBox.focus();

                        // Create a File object from the data URL
                        fetch('%s')
                            .then(res => res.blob())
                            .then(blob => {
                                const file = new File([blob], 'clipboard_image.png', { type: '%s' });

                                // Create a DataTransfer object to simulate drag and drop
                                const dt = new DataTransfer();
                                dt.items.add(file);

                                // Create and dispatch a paste event
                                const pasteEvent = new ClipboardEvent('paste', {
                                    clipboardData: dt,
                                    bubbles: true,
                                    cancelable: true
                                });

                                // Dispatch to the message box
                                messageBox.dispatchEvent(pasteEvent);

                                console.log('Image paste event dispatched to WhatsApp');
                            })
                            .catch(err => {
                                console.error('Failed to create file from data URL:', err);

                                // Fallback: try to trigger file input click
                                const fileInput = document.querySelector('input[type="file"][accept*="image"]');
                                if (fileInput) {
                                    // Create a new file input with our image
                                    const newFileInput = document.createElement('input');
                                    newFileInput.type = 'file';
                                    newFileInput.accept = 'image/*';
                                    newFileInput.style.display = 'none';
                                    document.body.appendChild(newFileInput);

                                    // We can't programmatically set files on input elements for security reasons
                                    // So this approach won't work either
                                    console.log('Cannot programmatically set file input - security restriction');
                                    document.body.removeChild(newFileInput);
                                }
                            });
                    } catch (error) {
                        console.error('Error injecting image into WhatsApp:', error);
                    }
                })();
            """.printf(data_url, mime_type);

            // Execute the JavaScript
            web_view.evaluate_javascript.begin(javascript, -1, null, null, null, (obj, res) => {
                try {
                    web_view.evaluate_javascript.end(res);
                    info("Image injection JavaScript executed successfully");
                    paste_succeeded(mime_type);
                } catch (Error e) {
                    critical("Failed to execute image injection JavaScript: %s", e.message);
                    paste_failed(_("Failed to paste image to WhatsApp"));
                }
            });
        }

    }
}
