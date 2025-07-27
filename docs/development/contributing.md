# Contributing to Karere

> Guide for contributors and community members

## Welcome Contributors!

Thank you for your interest in contributing to Karere! This guide will help you understand how to participate in the development of this native WhatsApp Web client for Linux.

## Table of Contents

- [Getting Started](#getting-started)
- [Types of Contributions](#types-of-contributions)
- [Development Setup](#development-setup)
- [Contribution Workflow](#contribution-workflow)
- [Code Guidelines](#code-guidelines)
- [Testing Requirements](#testing-requirements)
- [Documentation](#documentation)
- [Community Guidelines](#community-guidelines)

## Getting Started

### Before You Begin

1. **Read the Documentation**: Familiarize yourself with the project by reading:
   - [Project Mission](../../.agent-os/product/mission.md)
   - [Technical Architecture](../../.agent-os/product/tech-stack.md)
   - [Development Roadmap](../../.agent-os/product/roadmap.md)

2. **Understand the Codebase**: Review the source code structure and existing implementations

3. **Check Existing Issues**: Browse [GitHub Issues](https://github.com/tobagin/karere-vala/issues) to see what needs work

4. **Join the Community**: Participate in [GitHub Discussions](https://github.com/tobagin/karere-vala/discussions)

### First-Time Contributors

If you're new to open source or this project:

1. **Look for "Good First Issue" labels** on GitHub
2. **Start with documentation improvements** or small bug fixes
3. **Ask questions** in discussions or issue comments
4. **Read other contributors' code** to understand patterns

## Types of Contributions

### Code Contributions

#### Bug Fixes
- Fix reported issues
- Improve error handling
- Resolve performance problems
- Address security concerns

#### New Features
- Implement items from the roadmap
- Add user-requested functionality
- Improve existing features
- Enhance accessibility

#### Performance Improvements
- Optimize memory usage
- Reduce CPU consumption
- Improve startup time
- Enhance network efficiency

### Non-Code Contributions

#### Documentation
- User guides and tutorials
- API documentation
- Code comments and inline docs
- Translation of documentation

#### Testing
- Write unit tests
- Create integration tests
- Perform manual testing
- Report bugs with detailed reproduction steps

#### Design and UX
- UI/UX improvements
- Icon and graphics design
- Accessibility enhancements
- User experience research

#### Community Support
- Help users in discussions
- Triage issues
- Review pull requests
- Moderate community spaces

## Development Setup

### Prerequisites

Ensure you have the development environment set up as described in the [Building Guide](building.md).

### Fork and Clone

1. **Fork the repository** on GitHub
2. **Clone your fork**:
   ```bash
   git clone https://github.com/YOUR_USERNAME/karere-vala.git
   cd karere-vala
   ```
3. **Add upstream remote**:
   ```bash
   git remote add upstream https://github.com/tobagin/karere-vala.git
   ```

### Development Build

Set up your development environment:

```bash
# Build development version
./scripts/build.sh --dev --install

# Test the build
flatpak run io.github.tobagin.karere.Devel
```

## Contribution Workflow

### 1. Planning Your Contribution

#### For Bug Fixes
1. **Reproduce the issue** thoroughly
2. **Check existing issues** to avoid duplicates
3. **Create an issue** if one doesn't exist
4. **Discuss approach** in the issue comments

#### For New Features
1. **Check the roadmap** to see if feature is planned
2. **Create a feature request** or join existing discussion
3. **Get consensus** on the approach before coding
4. **Consider creating a design document** for complex features

### 2. Development Process

#### Create a Feature Branch
```bash
# Update your fork
git fetch upstream
git checkout main
git merge upstream/main

# Create feature branch
git checkout -b feature/your-feature-name
# or
git checkout -b fix/issue-number-description
```

#### Make Your Changes
1. **Follow code style guidelines** (see below)
2. **Write tests** for new functionality
3. **Update documentation** as needed
4. **Test thoroughly** on different configurations

#### Commit Guidelines
Follow [Conventional Commits](https://www.conventionalcommits.org/) specification:

```bash
# Feature commits
git commit -m "feat(notifications): add notification sound settings"

# Bug fix commits
git commit -m "fix(webkit): resolve memory leak in web view"

# Documentation commits
git commit -m "docs(user-guide): add troubleshooting section"

# Style/refactor commits
git commit -m "style(preferences): improve dialog layout"
git commit -m "refactor(logger): extract logging utility class"
```

**Commit Message Format:**
```
type(scope): brief description

Longer explanation if needed.

- List important changes
- Reference issues: Fixes #123
- Include breaking changes: BREAKING CHANGE: ...
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or modifying tests
- `chore`: Build system, dependencies, etc.

### 3. Testing Your Changes

#### Required Tests
```bash
# Run unit tests
meson test -C build

# Test Flatpak build
./scripts/build.sh --dev --install

# Manual testing
flatpak run io.github.tobagin.karere.Devel
```

#### Test Different Scenarios
- **Different desktop environments** (GNOME, KDE, XFCE)
- **Various network conditions** (slow, unreliable connections)
- **Different screen sizes and DPI settings**
- **With and without notifications enabled**
- **Light and dark themes**

### 4. Submitting Your Contribution

#### Pre-submission Checklist
- [ ] Code follows project style guidelines
- [ ] All tests pass
- [ ] Documentation is updated
- [ ] Commit messages follow conventional format
- [ ] No merge conflicts with main branch
- [ ] Feature works with both development and production builds

#### Create Pull Request
1. **Push to your fork**:
   ```bash
   git push origin feature/your-feature-name
   ```

2. **Create pull request** on GitHub with:
   - **Descriptive title**: Brief summary of changes
   - **Detailed description**: What, why, and how
   - **Screenshots/videos**: For UI changes
   - **Testing instructions**: How to test your changes
   - **References**: Link to related issues

#### Pull Request Template
```markdown
## Summary
Brief description of what this PR does.

## Changes Made
- List of specific changes
- Another important change
- Reference to issue: Closes #123

## Testing
- [ ] Unit tests pass
- [ ] Manual testing completed
- [ ] Tested on GNOME
- [ ] Tested with notifications
- [ ] No accessibility regressions

## Screenshots/Videos
[If applicable, add screenshots or videos]

## Additional Notes
Any additional context or considerations.
```

### 5. Review Process

#### What to Expect
- **Initial review**: Maintainers will review within 1-2 weeks
- **Feedback incorporation**: You may need to make changes
- **Multiple review rounds**: Complex changes may require several iterations
- **Final approval**: At least one maintainer approval required

#### Responding to Feedback
- **Be responsive**: Address feedback promptly
- **Ask questions**: If feedback is unclear, ask for clarification
- **Make requested changes**: Implement suggested improvements
- **Update documentation**: If changes affect docs

#### Making Changes After Review
```bash
# Make requested changes
git add modified_files
git commit -m "fix: address review feedback"

# Push updates
git push origin feature/your-feature-name
```

## Code Guidelines

### Code Style

Follow the project's [code style guide](../../.agent-os/standards/code-style.md):

#### Vala Code Style
```vala
namespace Karere {
    public class ExampleClass : GLib.Object {
        private string _private_member;
        
        public string public_property { get; set; }
        
        public ExampleClass (string initial_value) {
            this.public_property = initial_value;
            this._private_member = "default";
        }
        
        public void example_method () {
            if (this.public_property != null) {
                // Method implementation
                var local_variable = this.public_property.up ();
                debug ("Processing: %s", local_variable);
            }
        }
        
        private void private_helper_method () {
            // Private method implementation
        }
    }
}
```

#### Blueprint UI Style
```blueprint
using Gtk 4.0;
using Adw 1;

template $ExampleDialog : Adw.Dialog {
    content-width: 400;
    content-height: 300;
    
    Adw.HeaderBar {
        title-widget: Adw.WindowTitle {
            title: _("Example Dialog");
        };
    }
    
    Adw.ToastOverlay toast_overlay {
        Gtk.Box {
            orientation: vertical;
            spacing: 12;
            margin-top: 24;
            margin-bottom: 24;
            margin-start: 24;
            margin-end: 24;
            
            Gtk.Label {
                label: _("Example content");
                wrap: true;
            }
            
            Gtk.Button primary_button {
                label: _("Primary Action");
                clicked => $on_primary_clicked();
                
                styles ["suggested-action"]
            }
        }
    }
}
```

### Architecture Guidelines

#### Follow GNOME Design Patterns
- Use LibAdwaita widgets and patterns
- Follow GNOME Human Interface Guidelines
- Implement proper accessibility support
- Use standard keyboard shortcuts

#### Code Organization
```
src/
├── application.vala      # Main application class
├── window.vala          # Main window
├── preferences.vala     # Preferences dialog
├── logger.vala          # Logging functionality
├── notification-manager.vala  # Notification handling
├── webkit-manager.vala  # WebKit integration
├── crash-reporter.vala  # Crash reporting
└── utils.vala          # Utility functions
```

#### Dependency Management
- Minimize external dependencies
- Use only well-maintained libraries
- Prefer GLib/GTK solutions over third-party libraries
- Document any new dependencies in meson.build

### Error Handling

#### Use Proper Error Handling
```vala
public bool save_configuration () throws IOError {
    try {
        var file = File.new_for_path (config_path);
        var stream = file.create (FileCreateFlags.REPLACE_DESTINATION);
        stream.write (config_data.data);
        return true;
    } catch (Error e) {
        warning ("Failed to save configuration: %s", e.message);
        throw new IOError.FAILED ("Configuration save failed");
    }
}
```

#### Logging Guidelines
```vala
// Use appropriate log levels
debug ("Debug information for developers");
info ("General information");
warning ("Potential issues that don't break functionality");
critical ("Serious errors that may cause problems");

// Include context in log messages
debug ("Loading configuration from: %s", config_file_path);
warning ("Failed to connect after %d attempts", retry_count);
```

## Testing Requirements

### Unit Tests

#### Write Tests for New Features
```vala
// tests/test_logger.vala
void test_logger_initialization () {
    var logger = new Karere.Logger ();
    assert_nonnull (logger);
    assert_true (logger.is_initialized);
}

void test_logger_file_creation () throws Error {
    var logger = new Karere.Logger ();
    logger.initialize ("/tmp/test-logs");
    
    var log_file = File.new_for_path ("/tmp/test-logs/karere.log");
    assert_true (log_file.query_exists ());
}
```

#### Test Coverage Requirements
- **New features**: Must include comprehensive tests
- **Bug fixes**: Include regression tests
- **Critical paths**: Ensure core functionality is tested
- **Edge cases**: Test error conditions and boundary cases

### Manual Testing

#### Required Manual Tests
1. **Installation**: Verify Flatpak installation works
2. **Basic functionality**: Ensure WhatsApp Web loads and works
3. **Notifications**: Test native notification system
4. **Preferences**: Verify all settings work correctly
5. **Theme integration**: Test with different themes
6. **Performance**: Ensure no significant performance regression

#### Testing Checklist Template
```markdown
## Manual Testing Checklist

### Basic Functionality
- [ ] Application launches successfully
- [ ] WhatsApp Web interface loads
- [ ] Can log in with QR code
- [ ] Can send and receive messages
- [ ] Window resizing works correctly

### Notifications
- [ ] Notifications appear for new messages
- [ ] Notification sounds work (if enabled)
- [ ] Do Not Disturb integration works
- [ ] Notification actions work (if supported)

### Preferences
- [ ] Can open preferences dialog
- [ ] Theme changes apply correctly
- [ ] Notification settings work
- [ ] Privacy settings function properly

### Performance
- [ ] No significant memory leaks
- [ ] CPU usage reasonable
- [ ] Application responsive
- [ ] No crashes during normal use
```

## Documentation

### Code Documentation

#### Document Public APIs
```vala
/**
 * Manages application-wide notification handling.
 * 
 * The NotificationManager coordinates between WhatsApp Web notifications
 * and the native desktop notification system, providing seamless
 * integration with the user's desktop environment.
 */
public class NotificationManager : GLib.Object {
    /**
     * Initializes the notification manager.
     * 
     * @param application The main application instance
     * @throws Error if notification system cannot be initialized
     */
    public NotificationManager (Karere.Application application) throws Error {
        // Implementation
    }
    
    /**
     * Sends a desktop notification.
     * 
     * @param title The notification title
     * @param body The notification body text
     * @param icon The notification icon name
     * @return true if notification was sent successfully
     */
    public bool send_notification (string title, string body, string? icon = null) {
        // Implementation
    }
}
```

#### Update User Documentation
When making user-facing changes:
- Update relevant user guide sections
- Add new features to features documentation
- Include troubleshooting information if needed
- Update FAQ if introducing common questions

### Documentation Standards
- Use clear, concise language
- Include code examples for technical docs
- Add screenshots for UI changes
- Keep documentation up-to-date with code changes

## Community Guidelines

### Code of Conduct

We follow the [GNOME Code of Conduct](https://wiki.gnome.org/Foundation/CodeOfConduct). In summary:

- **Be respectful**: Treat all community members with respect
- **Be inclusive**: Welcome contributors of all backgrounds
- **Be constructive**: Provide helpful feedback and suggestions
- **Be professional**: Maintain professionalism in all interactions

### Communication

#### GitHub Discussions
- **General questions**: Use discussions for help and support
- **Feature ideas**: Discuss new features before implementation
- **Technical discussions**: Debate technical approaches and designs

#### Issue Tracker
- **Bug reports**: Use issues for reproducible bugs
- **Feature requests**: Create issues for specific feature requests
- **Project planning**: Track development tasks and milestones

#### Pull Request Reviews
- **Be constructive**: Provide helpful, actionable feedback
- **Explain rationale**: Explain why changes are needed
- **Suggest alternatives**: Offer better approaches when possible
- **Acknowledge good work**: Recognize quality contributions

### Recognition

Contributors are recognized through:
- **Contributor credits** in the about dialog
- **Changelog mentions** for significant contributions
- **GitHub contributor statistics**
- **Special recognition** for major contributions

## Getting Help

### Where to Ask Questions

1. **GitHub Discussions**: General questions and community support
2. **Issue Comments**: Questions about specific bugs or features
3. **Pull Request Comments**: Questions about code review feedback
4. **Matrix/Discord**: Real-time chat (check README for current links)

### What to Include When Asking for Help

- **Clear description** of what you're trying to do
- **Error messages** or unexpected behavior
- **Steps to reproduce** the issue
- **System information** (OS, desktop environment, versions)
- **Code samples** or configuration details

### Mentorship

New contributors can request mentorship:
- **Find a mentor** among existing contributors
- **Pair programming** sessions for complex features
- **Code review guidance** for first contributions
- **Project onboarding** assistance

## Advanced Contributing

### Becoming a Maintainer

Active contributors may be invited to become maintainers with:
- **Commit access** to the repository
- **Review responsibilities** for pull requests
- **Release management** duties
- **Community moderation** privileges

### Specialized Contributions

#### Platform Support
- **Distribution packaging** (RPM, DEB, etc.)
- **Desktop environment integration** improvements
- **Accessibility** enhancements
- **Internationalization** and localization

#### Technical Leadership
- **Architecture decisions** and technical direction
- **Performance optimization** initiatives
- **Security reviews** and improvements
- **Integration** with external services

---

*Thank you for contributing to Karere! Your contributions help make WhatsApp Web better for Linux desktop users everywhere.*