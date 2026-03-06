//! OKLCH design-system tokens shared across all strike48 apps.
//!
//! Provides class-based light/dark mode via `.dark` on `<html>`.
//! On first load a small JS snippet reads `localStorage("theme")`
//! and falls back to `prefers-color-scheme` so the user's choice
//! persists across reloads while still respecting the OS default.
//!
//! Token names follow the shadcn/ui convention used by strike48/ui
//! and dioxus-connector.

/// CSS custom properties (oklch colour tokens) with class-based
/// light/dark mode support, base resets, and hidden scrollbars.
pub fn theme_css() -> &'static str {
    r#"
        :root {
            color-scheme: light dark;

            /* shadcn/ui base tokens – light mode */
            --background: oklch(1 0 0);
            --foreground: oklch(0.145 0 0);
            --card: oklch(1 0 0);
            --popover: oklch(1 0 0);
            --primary: oklch(0.205 0 0);
            --primary-foreground: oklch(0.985 0 0);
            --secondary: oklch(0.97 0 0);
            --secondary-foreground: oklch(0.205 0 0);
            --muted: oklch(0.97 0 0);
            --muted-foreground: oklch(0.556 0 0);
            --accent: oklch(0.97 0 0);
            --accent-foreground: oklch(0.205 0 0);
            --destructive: oklch(0.5757 0.2352 27.92);
            --border: oklch(0.922 0 0);
            --input: oklch(0.922 0 0);
            --ring: oklch(0.708 0 0);
            --radius: 0.625rem;

            /* Sidebar tokens */
            --sidebar: oklch(0.985 0 0);
            --sidebar-foreground: oklch(0.145 0 0);
            --sidebar-primary: oklch(0.205 0 0);
            --sidebar-primary-foreground: oklch(0.985 0 0);
            --sidebar-accent: oklch(0.97 0 0);
            --sidebar-accent-foreground: oklch(0.205 0 0);
            --sidebar-border: oklch(0.922 0 0);
            --sidebar-ring: oklch(0.708 0 0);

            /* Chart tokens */
            --chart-1: oklch(0.646 0.222 41.116);
            --chart-2: oklch(0.6 0.118 184.704);
            --chart-3: oklch(0.398 0.07 227.392);
            --chart-4: oklch(0.828 0.189 84.429);
            --chart-5: oklch(0.769 0.188 70.08);

            /* Radius variants */
            --radius-sm: calc(var(--radius) - 4px);
            --radius-md: calc(var(--radius) - 2px);
            --radius-lg: var(--radius);
            --radius-xl: calc(var(--radius) + 4px);

            /* Extended tokens */
            --success: oklch(0.55 0.18 145);
            --warning: oklch(0.75 0.15 85);
            --info: oklch(0.6 0.15 250);

            /* Terminal/editor surface – slightly darker than --background */
            --surface: oklch(0.97 0 0);

            /* Typography tokens */
            --font-sans: ui-sans-serif, system-ui, sans-serif, 'Apple Color Emoji', 'Segoe UI Emoji', 'Segoe UI Symbol', 'Noto Color Emoji';
            --font-heading: ui-sans-serif, system-ui, sans-serif, 'Apple Color Emoji', 'Segoe UI Emoji', 'Segoe UI Symbol', 'Noto Color Emoji';
            --font-mono: "Cascadia Code", "Fira Code", "Consolas", "Courier New", monospace;
            --font-size: 14px;
        }

        .dark {
            color-scheme: dark;

            --background: oklch(0.145 0 0);
            --foreground: oklch(0.985 0 0);
            --card: oklch(0.145 0 0);
            --popover: oklch(0.145 0 0);
            --primary: oklch(0.985 0 0);
            --primary-foreground: oklch(0.205 0 0);
            --secondary: oklch(0.269 0 0);
            --secondary-foreground: oklch(0.985 0 0);
            --muted: oklch(0.269 0 0);
            --muted-foreground: oklch(0.708 0 0);
            --accent: oklch(0.269 0 0);
            --accent-foreground: oklch(0.985 0 0);
            --destructive: oklch(0.5058 0.2066 27.85);
            --border: oklch(0.269 0 0);
            --input: oklch(0.269 0 0);
            --ring: oklch(0.439 0 0);

            /* Sidebar tokens */
            --sidebar: oklch(0.205 0 0);
            --sidebar-foreground: oklch(0.985 0 0);
            --sidebar-primary: oklch(0.488 0.243 264.376);
            --sidebar-primary-foreground: oklch(0.985 0 0);
            --sidebar-accent: oklch(0.269 0 0);
            --sidebar-accent-foreground: oklch(0.985 0 0);
            --sidebar-border: oklch(0.269 0 0);
            --sidebar-ring: oklch(0.439 0 0);

            /* Chart tokens */
            --chart-1: oklch(0.488 0.243 264.376);
            --chart-2: oklch(0.696 0.17 162.48);
            --chart-3: oklch(0.769 0.188 70.08);
            --chart-4: oklch(0.627 0.265 303.9);
            --chart-5: oklch(0.645 0.246 16.439);

            --success: oklch(0.75 0.18 145);
            --warning: oklch(0.85 0.13 85);
            --info: oklch(0.7 0.15 250);

            /* Terminal/editor surface – slightly darker than --background */
            --surface: oklch(0.1 0 0);
        }

        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }

        /* Hidden scrollbars globally */
        *::-webkit-scrollbar {
            display: none;
        }
        * {
            scrollbar-width: none;
        }

        body {
            font-family: var(--font-sans);
            font-size: var(--font-size);
            background-color: var(--background);
            color: var(--foreground);
            line-height: 1.5;
        }
    "#
}

/// Small JS snippet that runs once on page load to apply the saved
/// theme (or the OS default) before the first paint.
pub fn theme_init_script() -> &'static str {
    r#"
    (function() {
        var stored = localStorage.getItem('theme');
        var prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        var dark = stored ? stored === 'dark' : prefersDark;
        if (dark) {
            document.documentElement.classList.add('dark');
        } else {
            document.documentElement.classList.remove('dark');
        }
    })();
    "#
}
