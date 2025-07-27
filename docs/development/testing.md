# Testing Guide

> Comprehensive guide to testing Karere for developers and contributors

## Overview

This guide covers all aspects of testing Karere, from automated unit tests to manual testing procedures. Proper testing ensures reliability, performance, and user experience quality.

## Testing Philosophy

### Testing Principles

1. **Test-Driven Development (TDD)**: Write tests before implementation when possible
2. **Comprehensive Coverage**: Test both happy paths and edge cases
3. **Automated Testing**: Prefer automated tests for regression prevention
4. **Manual Testing**: Use manual testing for user experience validation
5. **Performance Testing**: Monitor resource usage and performance metrics

### Testing Pyramid

```
    /\
   /  \    E2E Tests (Few)
  /____\   
 /      \   Integration Tests (Some)
/________\  Unit Tests (Many)
```

- **Unit Tests**: Test individual components in isolation
- **Integration Tests**: Test component interactions
- **End-to-End Tests**: Test complete user workflows

## Test Environment Setup

### Development Environment

#### Prerequisites
```bash
# Install testing dependencies
sudo apt install \
  gdb \
  valgrind \
  lcov \
  xvfb \
  dbus-x11

# For GUI testing
sudo apt install \
  python3-dogtail \
  python3-pyatspi
```

#### Build with Testing Support
```bash
# Configure build with testing enabled
meson setup build --buildtype=debug -Dtests=true

# Compile with test support
meson compile -C build
```

### Test Data and Fixtures

#### Test Data Directory
```
tests/
├── fixtures/           # Test data and mock files
│   ├── mock-responses/ # Mock network responses
│   ├── test-configs/   # Test configuration files
│   └── sample-data/    # Sample application data
├── integration/        # Integration test suites
├── unit/              # Unit test files
└── e2e/               # End-to-end test scenarios
```

#### Mock Data Setup
```bash
# Create test fixture directory
mkdir -p tests/fixtures/mock-responses

# Sample mock WhatsApp Web response
cat > tests/fixtures/mock-responses/login.json << 'EOF'
{
  "status": "success",
  "qr": "mock-qr-code-data",
  "timestamp": 1234567890
}
EOF
```

## Unit Testing

### Vala Unit Tests

#### Test Structure
```vala
// tests/unit/test_logger.vala
using Karere;

namespace KarereTests {
    class LoggerTest : Object {
        private Logger logger;
        private string temp_log_dir;
        
        public void setup () {
            this.temp_log_dir = DirUtils.make_tmp ("karere-test-XXXXXX");
            this.logger = new Logger ();
        }
        
        public void teardown () {
            // Cleanup test data
            DirUtils.remove (this.temp_log_dir);
        }
        
        public void test_logger_initialization () {
            assert (this.logger != null);
            assert (this.logger.is_initialized == false);
        }
        
        public void test_logger_file_creation () throws Error {
            this.logger.initialize (this.temp_log_dir);
            
            var log_file = File.new_for_path (
                Path.build_filename (this.temp_log_dir, "karere.log")
            );
            
            assert (log_file.query_exists ());
            assert (this.logger.is_initialized == true);
        }
        
        public void test_logger_write_message () throws Error {
            this.logger.initialize (this.temp_log_dir);
            this.logger.log (LogLevel.INFO, "Test message");
            
            var log_content = "";
            var log_file = File.new_for_path (
                Path.build_filename (this.temp_log_dir, "karere.log")
            );
            
            var stream = log_file.read ();
            var data_stream = new DataInputStream (stream);
            string line;
            
            while ((line = data_stream.read_line (null)) != null) {
                log_content += line + "\n";
            }
            
            assert ("Test message" in log_content);
            assert ("INFO" in log_content);
        }
    }
}

void main (string[] args) {
    Test.init (ref args);
    
    var test_suite = new KarereTests.LoggerTest ();
    
    Test.add_func ("/karere/logger/initialization", 
                   test_suite.test_logger_initialization);
    Test.add_func ("/karere/logger/file_creation", 
                   test_suite.test_logger_file_creation);
    Test.add_func ("/karere/logger/write_message", 
                   test_suite.test_logger_write_message);
    
    Test.run ();
}
```

#### Test Registration in Meson
```meson
# tests/unit/meson.build
test_logger = executable(
    'test_logger',
    'test_logger.vala',
    dependencies: [
        karere_deps,
        dependency('gio-2.0'),
    ],
    install: false
)

test('Logger Unit Tests', test_logger, suite: 'unit')
```

### Testing Different Components

#### Application Class Testing
```vala
// tests/unit/test_application.vala
public void test_application_creation () {
    var app = new Karere.Application ();
    assert (app != null);
    assert (app.application_id == "io.github.tobagin.karere.Devel");
}

public void test_application_flags () {
    var app = new Karere.Application ();
    assert (ApplicationFlags.HANDLES_OPEN in app.flags);
}
```

#### Window Testing
```vala
// tests/unit/test_window.vala
public void test_window_creation () {
    var app = new Karere.Application ();
    var window = new Karere.Window (app);
    
    assert (window != null);
    assert (window.application == app);
    assert (window.default_width > 0);
    assert (window.default_height > 0);
}
```

#### WebKit Manager Testing
```vala
// tests/unit/test_webkit_manager.vala
public void test_webkit_manager_initialization () {
    var manager = new Karere.WebKitManager ();
    assert (manager != null);
    assert (manager.web_view != null);
}

public void test_user_agent_setting () {
    var manager = new Karere.WebKitManager ();
    var settings = manager.web_view.get_settings ();
    
    var user_agent = settings.user_agent;
    assert (user_agent != null);
    assert ("Karere" in user_agent);
}
```

### Mock Objects and Test Doubles

#### Network Mock
```vala
// tests/mocks/mock_network.vala
public class MockNetworkSession : Object {
    private Queue<string> mock_responses;
    
    public MockNetworkSession () {
        this.mock_responses = new Queue<string> ();
    }
    
    public void add_mock_response (string response) {
        this.mock_responses.push_tail (response);
    }
    
    public async string fetch_url (string url) throws Error {
        if (this.mock_responses.is_empty ()) {
            throw new IOError.FAILED ("No mock response available");
        }
        
        return this.mock_responses.pop_head ();
    }
}
```

#### Configuration Mock
```vala
// tests/mocks/mock_config.vala
public class MockConfiguration : Karere.Configuration {
    private HashTable<string, Variant> mock_values;
    
    public MockConfiguration () {
        this.mock_values = new HashTable<string, Variant> (str_hash, str_equal);
    }
    
    public void set_mock_value (string key, Variant value) {
        this.mock_values.set (key, value);
    }
    
    public override Variant get_value (string key) throws Error {
        var mock_value = this.mock_values.get (key);
        if (mock_value != null) {
            return mock_value;
        }
        
        return base.get_value (key);
    }
}
```

## Integration Testing

### Component Integration Tests

#### WebKit Integration Test
```vala
// tests/integration/test_webkit_integration.vala
public class WebKitIntegrationTest : Object {
    private Karere.Application app;
    private Karere.Window window;
    private MainLoop main_loop;
    
    public void setup () {
        this.app = new Karere.Application ();
        this.window = new Karere.Window (this.app);
        this.main_loop = new MainLoop ();
    }
    
    public void test_whatsapp_web_loading () {
        var webkit_manager = this.window.webkit_manager;
        var web_view = webkit_manager.web_view;
        
        bool load_finished = false;
        web_view.load_changed.connect ((load_event) => {
            if (load_event == WebKit.LoadEvent.FINISHED) {
                load_finished = true;
                this.main_loop.quit ();
            }
        });
        
        webkit_manager.load_whatsapp_web ();
        
        // Run main loop with timeout
        Timeout.add_seconds (30, () => {
            this.main_loop.quit ();
            return false;
        });
        
        this.main_loop.run ();
        assert (load_finished);
    }
}
```

#### Notification Integration Test
```vala
// tests/integration/test_notification_integration.vala
public void test_notification_display () {
    var app = new Karere.Application ();
    var notification_manager = new Karere.NotificationManager (app);
    
    bool notification_sent = false;
    notification_manager.notification_sent.connect (() => {
        notification_sent = true;
    });
    
    notification_manager.show_notification (
        "Test Title",
        "Test message content",
        "dialog-information"
    );
    
    // Allow time for notification processing
    Thread.usleep (1000000); // 1 second
    assert (notification_sent);
}
```

### Database Integration Tests

#### Configuration Storage Test
```vala
// tests/integration/test_config_storage.vala
public void test_configuration_persistence () throws Error {
    var temp_dir = DirUtils.make_tmp ("karere-config-test-XXXXXX");
    var config = new Karere.Configuration (temp_dir);
    
    // Write configuration
    config.set_string ("theme", "dark");
    config.set_boolean ("notifications_enabled", true);
    config.set_integer ("window_width", 1200);
    
    // Create new instance to test persistence
    var config2 = new Karere.Configuration (temp_dir);
    
    assert (config2.get_string ("theme") == "dark");
    assert (config2.get_boolean ("notifications_enabled") == true);
    assert (config2.get_integer ("window_width") == 1200);
    
    // Cleanup
    DirUtils.remove (temp_dir);
}
```

## End-to-End Testing

### Automated GUI Testing

#### Using Dogtail for GUI Testing
```python
#!/usr/bin/env python3
# tests/e2e/test_basic_functionality.py

import unittest
import subprocess
import time
from dogtail.tree import root
from dogtail.utils import isA11yEnabled, enableA11y

class KarereE2ETest(unittest.TestCase):
    def setUp(self):
        # Ensure accessibility is enabled
        if not isA11yEnabled():
            enableA11y(True)
        
        # Launch Karere
        self.karere_process = subprocess.Popen([
            'flatpak', 'run', 'io.github.tobagin.karere.Devel'
        ])
        
        # Wait for application to start
        time.sleep(5)
        
        # Find Karere window
        self.karere = root.application('karere')
        self.window = self.karere.window('Karere')
    
    def tearDown(self):
        # Close application
        self.karere_process.terminate()
        self.karere_process.wait()
    
    def test_application_starts(self):
        """Test that Karere starts successfully"""
        self.assertTrue(self.window.showing)
        self.assertEqual(self.window.name, 'Karere')
    
    def test_preferences_dialog(self):
        """Test opening preferences dialog"""
        # Use keyboard shortcut to open preferences
        self.window.keyCombo('<ctrl>comma')
        
        # Wait for preferences dialog
        time.sleep(2)
        
        preferences_dialog = self.karere.dialog('Preferences')
        self.assertTrue(preferences_dialog.showing)
        
        # Close preferences
        preferences_dialog.keyCombo('Escape')
    
    def test_theme_switching(self):
        """Test theme switching functionality"""
        # Open preferences
        self.window.keyCombo('<ctrl>comma')
        time.sleep(1)
        
        preferences_dialog = self.karere.dialog('Preferences')
        
        # Navigate to appearance settings
        appearance_page = preferences_dialog.child('Appearance')
        appearance_page.click()
        
        # Find theme selector
        theme_selector = preferences_dialog.child('Theme')
        
        # Test switching to dark theme
        dark_option = theme_selector.child('Dark')
        dark_option.click()
        
        # Verify theme changed (implementation depends on how theme is exposed)
        # This would need to check actual theme application
        
        preferences_dialog.keyCombo('Escape')

if __name__ == '__main__':
    unittest.main()
```

#### Running GUI Tests
```bash
# Run GUI tests with virtual display
xvfb-run -a python3 tests/e2e/test_basic_functionality.py

# Run with visible display for debugging
DISPLAY=:0 python3 tests/e2e/test_basic_functionality.py
```

### Manual Testing Scenarios

#### Critical Path Testing
```markdown
# tests/manual/critical_paths.md

## Critical Path Test Scenarios

### 1. Application Startup
- [ ] Application launches without errors
- [ ] Main window appears with correct size
- [ ] WebKit view initializes properly
- [ ] WhatsApp Web begins loading

### 2. WhatsApp Web Integration
- [ ] WhatsApp Web loads completely
- [ ] QR code appears for login
- [ ] Can scan QR code with phone
- [ ] Successfully logs into WhatsApp
- [ ] Chat interface displays correctly

### 3. Notification System
- [ ] Notifications appear for new messages
- [ ] Notification content is correct
- [ ] Notification sounds work (if enabled)
- [ ] Clicking notification opens relevant chat
- [ ] Do Not Disturb integration works

### 4. Preferences
- [ ] Preferences dialog opens
- [ ] All preference categories accessible
- [ ] Theme changes apply immediately
- [ ] Notification settings work
- [ ] Privacy settings function correctly
- [ ] Settings persist after restart

### 5. Performance
- [ ] Application starts within reasonable time
- [ ] Memory usage stays within acceptable limits
- [ ] CPU usage reasonable during idle
- [ ] No significant memory leaks
- [ ] Responsive during normal usage
```

## Performance Testing

### Memory Usage Testing

#### Memory Leak Detection
```bash
# Run with Valgrind for memory leak detection
valgrind --tool=memcheck \
  --leak-check=full \
  --show-leak-kinds=all \
  --track-origins=yes \
  --log-file=valgrind-output.txt \
  ./build/src/karere

# Analyze results
grep "definitely lost" valgrind-output.txt
grep "possibly lost" valgrind-output.txt
```

#### Memory Usage Monitoring
```bash
#!/bin/bash
# tests/performance/monitor_memory.sh

KARERE_PID=$(pgrep karere)
if [ -z "$KARERE_PID" ]; then
    echo "Karere not running"
    exit 1
fi

echo "Monitoring memory usage for Karere (PID: $KARERE_PID)"
echo "Time,RSS,VSZ,CPU" > memory_usage.csv

while kill -0 $KARERE_PID 2>/dev/null; do
    STATS=$(ps -p $KARERE_PID -o rss,vsz,pcpu --no-headers)
    TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
    echo "$TIMESTAMP,$STATS" >> memory_usage.csv
    sleep 10
done
```

### Performance Benchmarks

#### Startup Time Measurement
```vala
// tests/performance/startup_benchmark.vala
public class StartupBenchmark : Object {
    public void measure_startup_time () {
        var start_time = get_monotonic_time ();
        
        var app = new Karere.Application ();
        app.activate ();
        
        var end_time = get_monotonic_time ();
        var startup_time = (end_time - start_time) / 1000; // Convert to milliseconds
        
        print ("Startup time: %ld ms\n", startup_time);
        
        // Assert reasonable startup time (adjust threshold as needed)
        assert (startup_time < 5000); // Less than 5 seconds
    }
}
```

#### Network Performance Testing
```bash
#!/bin/bash
# tests/performance/network_benchmark.sh

echo "Testing network performance..."

# Measure time to load WhatsApp Web
curl -w "@curl-format.txt" -o /dev/null -s "https://web.whatsapp.com/"

# curl-format.txt content:
#     time_namelookup:  %{time_namelookup}\n
#        time_connect:  %{time_connect}\n
#     time_appconnect:  %{time_appconnect}\n
#    time_pretransfer:  %{time_pretransfer}\n
#       time_redirect:  %{time_redirect}\n
#  time_starttransfer:  %{time_starttransfer}\n
#                     ----------\n
#          time_total:  %{time_total}\n
```

## Test Automation

### Continuous Integration Testing

#### GitHub Actions Workflow
```yaml
# .github/workflows/test.yml
name: Test Suite

on:
  push:
    branches: [ main, development ]
  pull_request:
    branches: [ main ]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Install dependencies
      run: |
        sudo apt update
        sudo apt install -y \
          meson \
          valac \
          libgtk-4-dev \
          libadwaita-1-dev \
          libwebkitgtk-6.0-dev \
          blueprint-compiler
    
    - name: Setup build
      run: meson setup build --buildtype=debug -Dtests=true
    
    - name: Compile
      run: meson compile -C build
    
    - name: Run unit tests
      run: meson test -C build --suite unit
    
    - name: Generate coverage report
      run: |
        ninja -C build coverage
        bash <(curl -s https://codecov.io/bash)

  integration-tests:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Install dependencies
      run: |
        sudo apt update
        sudo apt install -y \
          meson \
          valac \
          libgtk-4-dev \
          libadwaita-1-dev \
          libwebkitgtk-6.0-dev \
          blueprint-compiler \
          xvfb
    
    - name: Setup build
      run: meson setup build --buildtype=debug -Dtests=true
    
    - name: Compile
      run: meson compile -C build
    
    - name: Run integration tests
      run: xvfb-run -a meson test -C build --suite integration

  flatpak-build-test:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Install Flatpak
      run: |
        sudo apt update
        sudo apt install -y flatpak flatpak-builder
        sudo flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
    
    - name: Build Flatpak
      run: |
        flatpak-builder --install-deps-from=flathub --force-clean \
          build-dir packaging/io.github.tobagin.karere.Devel.yml
    
    - name: Test Flatpak installation
      run: |
        flatpak-builder --user --install --force-clean \
          build-dir packaging/io.github.tobagin.karere.Devel.yml
        
        # Basic smoke test
        timeout 30 flatpak run io.github.tobagin.karere.Devel --version
```

### Test Data Management

#### Test Database Setup
```sql
-- tests/fixtures/test_schema.sql
CREATE TABLE IF NOT EXISTS test_configurations (
    id INTEGER PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    value TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO test_configurations (key, value) VALUES
    ('theme', 'dark'),
    ('notifications_enabled', 'true'),
    ('window_width', '1200'),
    ('window_height', '800');
```

#### Test Configuration Files
```ini
# tests/fixtures/test_config.ini
[general]
debug=true
log_level=debug

[window]
width=800
height=600
maximized=false

[notifications]
enabled=true
sounds=false
preview=true
```

## Testing Best Practices

### Test Organization

#### Directory Structure
```
tests/
├── unit/                    # Unit tests
│   ├── test_application.vala
│   ├── test_window.vala
│   ├── test_logger.vala
│   ├── test_notifications.vala
│   └── meson.build
├── integration/             # Integration tests
│   ├── test_webkit_integration.vala
│   ├── test_notification_integration.vala
│   └── meson.build
├── e2e/                     # End-to-end tests
│   ├── test_basic_functionality.py
│   ├── test_preferences.py
│   └── requirements.txt
├── performance/             # Performance tests
│   ├── startup_benchmark.vala
│   ├── memory_monitor.sh
│   └── network_benchmark.sh
├── fixtures/                # Test data
│   ├── mock_responses/
│   ├── test_configs/
│   └── sample_data/
└── mocks/                   # Mock objects
    ├── mock_network.vala
    └── mock_config.vala
```

### Test Naming Conventions

```vala
// Good test names - describe what is being tested
public void test_logger_creates_file_on_initialization ()
public void test_notification_manager_sends_desktop_notification ()
public void test_preferences_dialog_saves_theme_selection ()

// Poor test names - unclear what is being tested
public void test_logger ()
public void test_notification ()
public void test_preferences ()
```

### Test Data Isolation

```vala
public class TestWithIsolation : Object {
    private string temp_dir;
    
    public void setup () {
        // Create isolated test environment
        this.temp_dir = DirUtils.make_tmp ("karere-test-XXXXXX");
        Environment.set_variable ("XDG_CONFIG_HOME", 
                                 Path.build_filename (this.temp_dir, "config"), 
                                 true);
        Environment.set_variable ("XDG_DATA_HOME", 
                                 Path.build_filename (this.temp_dir, "data"), 
                                 true);
    }
    
    public void teardown () {
        // Clean up test environment
        this.remove_directory_recursive (this.temp_dir);
    }
    
    private void remove_directory_recursive (string path) {
        var dir = File.new_for_path (path);
        try {
            var enumerator = dir.enumerate_children (
                FileAttribute.STANDARD_NAME,
                FileQueryInfoFlags.NOFOLLOW_SYMLINKS
            );
            
            FileInfo info;
            while ((info = enumerator.next_file ()) != null) {
                var child = dir.get_child (info.get_name ());
                if (info.get_file_type () == FileType.DIRECTORY) {
                    this.remove_directory_recursive (child.get_path ());
                } else {
                    child.delete ();
                }
            }
            
            dir.delete ();
        } catch (Error e) {
            warning ("Failed to remove test directory: %s", e.message);
        }
    }
}
```

## Debugging Tests

### Debug Test Failures

#### Running Single Tests
```bash
# Run specific test
meson test -C build test_logger --verbose

# Run test with debugger
meson test -C build test_logger --gdb

# Run test with custom arguments
meson test -C build test_logger --test-args="--debug"
```

#### Test Debugging Output
```vala
public void test_with_debug_output () {
    debug ("Starting test_with_debug_output");
    
    var logger = new Karere.Logger ();
    debug ("Created logger: %p", logger);
    
    assert (logger != null);
    debug ("Logger assertion passed");
    
    // Test implementation
    debug ("Test completed successfully");
}
```

### Test Environment Debugging

#### Check Test Environment
```bash
# Show test environment
meson test -C build --list

# Show test configuration
meson introspect build --tests

# Run tests with maximum verbosity
meson test -C build --verbose --print-errorlogs
```

## Coverage Analysis

### Code Coverage Setup

#### Enable Coverage in Build
```bash
# Build with coverage support
meson setup build --buildtype=debug -Db_coverage=true -Dtests=true
meson compile -C build
```

#### Generate Coverage Reports
```bash
# Run tests and generate coverage
meson test -C build
ninja -C build coverage

# View coverage report
genhtml build/meson-logs/coverage.info --output-directory coverage-html
firefox coverage-html/index.html
```

#### Coverage Targets
- **Unit Tests**: Aim for >90% line coverage
- **Integration Tests**: Focus on critical paths
- **Overall**: Maintain >80% total coverage

---

*For additional testing resources, see the [Building Guide](building.md) and [Contributing Guide](contributing.md).*