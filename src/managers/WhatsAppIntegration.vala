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
     * Handles integration with WhatsApp Web through JavaScript injection
     */
    public class WhatsAppIntegration : GLib.Object {
        private WebKit.WebView web_view;

        public signal void operation_completed(bool success, string message);

        public WhatsAppIntegration(WebKit.WebView web_view) {
            this.web_view = web_view;
        }

        /**
         * Inject an image into WhatsApp Web chat
         */
        public void inject_image(Bytes image_bytes, string mime_type) {
            info("Injecting %s image (%zu bytes) into WhatsApp Web", mime_type, image_bytes.get_size());

            var data_url = create_data_url_from_bytes(image_bytes, mime_type);
            var javascript = create_image_injection_script(data_url, mime_type);

            execute_javascript(javascript, (success) => {
                if (success) {
                    info("Image injection completed successfully");
                    operation_completed(true, _("Image pasted to WhatsApp"));
                } else {
                    critical("Image injection failed");
                    operation_completed(false, _("Failed to paste image to WhatsApp"));
                }
            });
        }

        /**
         * Execute default paste behavior (for non-image content)
         */
        public void execute_default_paste() {
            debug("Executing default paste behavior");

            var javascript = create_default_paste_script();
            execute_javascript(javascript, (success) => {
                if (success) {
                    debug("Default paste executed successfully");
                } else {
                    warning("Default paste execution failed");
                }
            });
        }

        /**
         * Create data URL from image bytes
         */
        private string create_data_url_from_bytes(Bytes image_bytes, string mime_type) {
            var base64_data = Base64.encode(image_bytes.get_data());
            return "data:%s;base64,%s".printf(mime_type, base64_data);
        }

        /**
         * Create JavaScript for image injection
         */
        private string create_image_injection_script(string data_url, string mime_type) {
            return """
                (function() {
                    try {
                        %s

                        if (!messageBox) {
                            console.log('WhatsApp message box not found');
                            return false;
                        }

                        // Focus the message box
                        messageBox.focus();

                        // Create and inject image
                        %s

                        return true;
                    } catch (error) {
                        console.error('Error injecting image into WhatsApp:', error);
                        return false;
                    }
                })();
            """.printf(
                get_message_box_finder_script(),
                get_image_paste_implementation(data_url, mime_type)
            );
        }

        /**
         * Create JavaScript for default paste
         */
        private string create_default_paste_script() {
            return """
                (function() {
                    try {
                        %s

                        if (messageBox) {
                            messageBox.focus();
                            document.execCommand('paste');
                            console.log('Default paste executed');
                            return true;
                        } else {
                            console.log('WhatsApp message box not found for default paste');
                            return false;
                        }
                    } catch (error) {
                        console.error('Error executing default paste:', error);
                        return false;
                    }
                })();
            """.printf(get_message_box_finder_script());
        }

        /**
         * Get script to find WhatsApp message box
         */
        private string get_message_box_finder_script() {
            return """
                const messageBox = document.querySelector('[contenteditable="true"][data-tab="10"]') ||
                                  document.querySelector('[contenteditable="true"]') ||
                                  document.querySelector('div[contenteditable="true"]');
            """;
        }

        /**
         * Get image paste implementation with fallback
         */
        private string get_image_paste_implementation(string data_url, string mime_type) {
            return """
                fetch('%s')
                    .then(res => res.blob())
                    .then(blob => {
                        const file = new File([blob], 'clipboard_image.png', { type: '%s' });

                        %s

                        messageBox.dispatchEvent(pasteEvent);
                        console.log('Image paste event dispatched to WhatsApp');
                    })
                    .catch(err => {
                        console.error('Failed to create file from data URL:', err);
                        %s
                    });
            """.printf(
                data_url,
                mime_type,
                get_paste_event_creation_script(),
                get_fallback_file_input_script()
            );
        }

        /**
         * Get script to create paste event
         */
        private string get_paste_event_creation_script() {
            return """
                const dt = new DataTransfer();
                dt.items.add(file);

                const pasteEvent = new ClipboardEvent('paste', {
                    clipboardData: dt,
                    bubbles: true,
                    cancelable: true
                });
            """;
        }

        /**
         * Get fallback file input script
         */
        private string get_fallback_file_input_script() {
            return """
                const fileInput = document.querySelector('input[type="file"][accept*="image"]');
                if (fileInput) {
                    const newFileInput = document.createElement('input');
                    newFileInput.type = 'file';
                    newFileInput.accept = 'image/*';
                    newFileInput.style.display = 'none';
                    document.body.appendChild(newFileInput);

                    console.log('Cannot programmatically set file input - security restriction');
                    document.body.removeChild(newFileInput);
                }
            """;
        }

        /**
         * Execute JavaScript code in the WebView
         */
        private void execute_javascript(string javascript, owned JavaScriptCallback? callback = null) {
            web_view.evaluate_javascript.begin(javascript, -1, null, null, null, (obj, res) => {
                try {
                    web_view.evaluate_javascript.end(res);
                    if (callback != null) {
                        callback(true);
                    }
                } catch (Error e) {
                    critical("Failed to execute JavaScript: %s", e.message);
                    if (callback != null) {
                        callback(false);
                    }
                }
            });
        }

        public delegate void JavaScriptCallback(bool success);

        /**
         * Check if current page is WhatsApp Web
         */
        public bool is_whatsapp_page() {
            var current_uri = web_view.get_uri();
            return current_uri != null &&
                   (current_uri.has_prefix(Constants.Network.WHATSAPP_DOMAINS[0]) ||
                    current_uri.has_prefix(Constants.Network.WHATSAPP_DOMAINS[1]) ||
                    current_uri.has_prefix(Constants.Network.WHATSAPP_DOMAINS[2]));
        }

        /**
         * Inject user agent override
         */
        public void inject_user_agent_override(string user_agent) {
            var javascript = """
                Object.defineProperty(navigator, 'userAgent', {
                    get: function() {
                        return '%s';
                    },
                    configurable: false,
                    enumerable: true
                });

                Object.defineProperty(navigator, 'platform', {
                    get: function() {
                        return 'Linux x86_64';
                    },
                    configurable: false,
                    enumerable: true
                });

                console.log('User agent overridden to:', navigator.userAgent);
            """.printf(user_agent);

            execute_javascript(javascript, (success) => {
                if (success) {
                    debug("User agent override injected successfully");
                } else {
                    warning("Failed to inject user agent override");
                }
            });
        }
    }
}