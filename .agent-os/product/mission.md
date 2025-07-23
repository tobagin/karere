# Product Mission

> Last Updated: 2025-01-23
> Version: 1.0.0

## Pitch

Karere is a modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that helps Linux desktop users communicate more seamlessly by providing a native, integrated experience that surpasses the limitations of using WhatsApp in a standard web browser.

## Users

### Primary Customers

- **Linux Desktop Users**: Individuals who prefer native applications over web-based solutions for their daily communication needs
- **GNOME Ecosystem Users**: Users who value consistent design language and native integration with their desktop environment

### User Personas

**Linux Power User** (25-45 years old)
- **Role:** Software Developer, System Administrator, or Tech Enthusiast
- **Context:** Uses Linux as primary desktop OS, values native applications and system integration
- **Pain Points:** Web-based WhatsApp lacks native notifications, doesn't integrate with desktop themes, consumes excessive browser resources
- **Goals:** Seamless communication experience, native desktop integration, efficient resource usage

**Privacy-Conscious Professional** (30-50 years old)
- **Role:** IT Professional, Security Researcher, or Business Owner
- **Context:** Requires reliable communication tools with transparency and control
- **Pain Points:** Browser-based solutions lack visibility into crashes and errors, limited debugging capabilities
- **Goals:** Stable communication platform with comprehensive logging and crash reporting

## The Problem

### Fragmented WhatsApp Experience on Linux

WhatsApp Web in browsers provides a suboptimal experience for Linux users, lacking native desktop integration, proper notification handling, and theme consistency. This results in a disconnected user experience that doesn't match the quality of native desktop applications.

**Our Solution:** Karere provides a native GTK4/LibAdwaita wrapper that seamlessly integrates WhatsApp Web with the Linux desktop environment.

### Poor Resource Management and Debugging

Browser-based WhatsApp Web offers no insight into performance issues or crashes, making troubleshooting difficult for users experiencing problems. Memory usage is often excessive due to browser overhead.

**Our Solution:** Comprehensive logging system and crash reporting provide transparency and debugging capabilities while optimized WebKitGTK rendering reduces resource consumption.

## Differentiators

### Native Desktop Integration

Unlike browser-based WhatsApp Web, Karere provides true native desktop notifications through GNotification, full theme support (Light, Dark, System), and consistent LibAdwaita design language. This results in seamless integration with the GNOME desktop environment.

### Comprehensive Observability

Unlike other WhatsApp wrappers that offer minimal debugging capabilities, Karere includes a full logging system and opt-in crash reporting. This provides users and developers with complete visibility into application behavior and performance.

### Developer-Focused Architecture

Unlike simple Electron-based wrappers, Karere uses native Vala and WebKitGTK 6.0 for optimal performance and memory efficiency while maintaining dual Flatpak manifests for streamlined development and production deployment.

## Key Features

### Core Features

- **WebKitGTK Integration:** Flawless rendering and operation of the WhatsApp Web interface using WebKitGTK 6.0
- **Native Notifications:** Intercept web notifications and display them using GNotification for true desktop integration
- **LibAdwaita UI Shell:** Minimal, modern window with HeaderBar supporting Light, Dark, and System themes
- **Full Logging System:** Comprehensive logging mechanism to capture application events and errors for debugging
- **Application Crash Reporter:** Opt-in system to detect crashes and allow users to submit reports for development

### Distribution Features

- **Dual Flatpak Configuration:** Separate production and development manifests for streamlined workflows
- **Native Package Management:** Full Flatpak integration with proper sandboxing and permissions