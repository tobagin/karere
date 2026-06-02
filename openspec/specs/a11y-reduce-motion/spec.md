# a11y-reduce-motion Specification

## Purpose

Allow users to disable application animations at runtime via a GSettings key bound to GtkSettings, reducing motion for accessibility.

## Requirements

### Requirement: Reduce-motion setting binds to GtkSettings
The application SHALL expose a boolean GSettings key `reduce-motion` (default `false`). When `true`, the application SHALL set `GtkSettings::gtk-enable-animations` to `false`; when `false`, it SHALL set it to `true`.

#### Scenario: Enabling reduce-motion disables animations
- **WHEN** the user sets `reduce-motion` to `true` at runtime
- **THEN** `GtkSettings::default()` has `gtk-enable-animations` set to `false`
- **AND** subsequent AdwAnimation transitions complete instantly and toast slide-ins are skipped

#### Scenario: Disabling reduce-motion restores animations
- **WHEN** `reduce-motion` transitions from `true` to `false`
- **THEN** `gtk-enable-animations` is set back to `true` without restarting the application

#### Scenario: Default preserves existing behavior
- **WHEN** the application starts with no prior value for `reduce-motion`
- **THEN** the key reads as `false` and `gtk-enable-animations` remains at its GTK default (`true`)
