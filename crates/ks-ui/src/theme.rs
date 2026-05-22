//! Strike48 Design System tokens for KubeStudio.
//!
//! Implements the Strike48 dense, dark-first console design language.
//! Provides class-based light/dark mode via `data-theme` attribute on `<html>`.
//!
//! Design philosophy:
//! - Dense over spacious
//! - Dark over light
//! - Data over decoration
//! - Keyboard-first
//! - 13px base font (not 16px)
//! - Monospace for machine data

/// CSS custom properties with Strike48 ink scale colors.
/// Dark mode is default, light mode via data-theme="strike48-light".
pub fn theme_css() -> &'static str {
    r#"
        /* ============================================
           STRIKE48 DESIGN SYSTEM TOKENS
           ============================================ */

        @import url('https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600;700&family=IBM+Plex+Sans:wght@400;500;600;700&display=swap');

        :root,
        [data-theme="strike48"] {
            color-scheme: dark;

            /* --- Ink Scale (surfaces & text) --- */
            --ink-900: #07090d;
            --ink-850: #0b0e14;
            --ink-800: #0f1320;
            --ink-750: #141a28;
            --ink-700: #1a2233;
            --ink-650: #222b40;
            --ink-600: #2c3753;
            --ink-500: #4a5578;
            --ink-400: #6e7a9a;
            --ink-300: #9ba4be;
            --ink-200: #cdd2e2;
            --ink-100: #eef0f7;

            /* --- Semantic Mappings --- */
            --background: var(--ink-900);
            --foreground: var(--ink-200);
            --card: var(--ink-800);
            --popover: var(--ink-800);
            --primary: var(--ink-100);
            --primary-foreground: var(--ink-900);
            --secondary: var(--ink-850);
            --secondary-foreground: var(--ink-200);
            --muted: var(--ink-700);
            --muted-foreground: var(--ink-400);
            --accent: var(--ink-700);
            --accent-foreground: var(--ink-100);
            --border: var(--ink-700);
            --input: var(--ink-700);
            --ring: var(--ink-600);
            --surface: var(--ink-850);

            /* --- Brand Colors --- */
            --brand-300: #7aa9ff;
            --brand-500: #3978D5;
            --brand-600: #2563eb;
            --brand-700: #1d4ed8;

            /* --- Accent (gold - use sparingly) --- */
            --accent-amber: #efbf04;
            --accent-amber-dark: #d9aa03;

            /* --- Status Colors --- */
            --status-critical: #ef4444;
            --status-high: #f97316;
            --status-medium: #3b82f6;
            --status-low: #64748b;
            --status-open: #3b82f6;
            --status-resolved: #10b981;
            --status-in-progress: #eab308;
            --status-waiting: #a855f7;
            --status-closed: #475569;

            /* --- Shadows --- */
            --shadow-subtle: 0 1px 3px rgba(0, 0, 0, 0.3);
            --shadow-overlay: 0 8px 24px rgba(0, 0, 0, 0.5);
            --shadow-gold: 8px 8px 0px #efbf04;
            --shadow-gold-hover: 12px 12px 0px #d9aa03;

            /* --- Mapped Status Tokens (for compatibility) --- */
            --success: #10b981;
            --warning: #eab308;
            --info: #3978D5;
            --destructive: #ef4444;

            /* --- Sidebar Tokens --- */
            --sidebar: var(--ink-850);
            --sidebar-foreground: var(--ink-200);
            --sidebar-primary: var(--brand-500);
            --sidebar-primary-foreground: var(--ink-100);
            --sidebar-accent: var(--ink-700);
            --sidebar-accent-foreground: var(--ink-100);
            --sidebar-border: var(--ink-700);
            --sidebar-ring: var(--ink-600);

            /* --- Chart Colors --- */
            --chart-1: var(--brand-500);
            --chart-2: var(--status-resolved);
            --chart-3: var(--status-in-progress);
            --chart-4: var(--status-waiting);
            --chart-5: var(--status-high);

            /* --- Radius (Strike48: max 4px) --- */
            --radius: 4px;
            --radius-sm: 2px;
            --radius-md: 4px;
            --radius-lg: 6px;

            /* --- Typography --- */
            --font-sans: 'IBM Plex Sans', ui-sans-serif, system-ui, sans-serif;
            --font-mono: 'IBM Plex Mono', ui-monospace, 'Cascadia Code', 'Fira Code', Consolas, monospace;
            --font-size: 13px;
        }

        /* --- Light Mode --- */
        [data-theme="strike48-light"] {
            color-scheme: light;

            /* --- Inverted Ink Scale --- */
            --ink-900: #F9FAFB;
            --ink-850: #F3F4F6;
            --ink-800: #FFFFFF;
            --ink-750: #F9FAFB;
            --ink-700: #E5E7EB;
            --ink-650: #D1D5DB;
            --ink-600: #9CA3AF;
            --ink-500: #6B7280;
            --ink-400: #6B7280;
            --ink-300: #4B5563;
            --ink-200: #1F2937;
            --ink-100: #111827;
        }

        /* ============================================
           BASE RESETS
           ============================================ */
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }

        html, body {
            height: 100%;
        }

        body {
            font-family: var(--font-sans);
            font-size: var(--font-size);
            font-feature-settings: 'cv11', 'ss01', 'ss03';
            -webkit-font-smoothing: antialiased;
            text-rendering: optimizeLegibility;
            background-color: var(--background);
            color: var(--foreground);
            line-height: 1.5;
        }

        /* ============================================
           SCROLLBARS (Strike48 style)
           ============================================ */
        *::-webkit-scrollbar {
            width: 8px;
            height: 8px;
        }

        *::-webkit-scrollbar-track {
            background: var(--ink-850);
        }

        *::-webkit-scrollbar-thumb {
            background: var(--ink-650);
            border-radius: 2px;
        }

        *::-webkit-scrollbar-thumb:hover {
            background: var(--ink-600);
        }

        /* Light mode scrollbars */
        [data-theme="strike48-light"] *::-webkit-scrollbar-track {
            background: #F3F4F6;
        }

        [data-theme="strike48-light"] *::-webkit-scrollbar-thumb {
            background: #D1D5DB;
        }

        [data-theme="strike48-light"] *::-webkit-scrollbar-thumb:hover {
            background: #9CA3AF;
        }

        /* ============================================
           SELECTION & FOCUS
           ============================================ */
        ::selection {
            background: rgba(37, 99, 235, 0.33);
            color: var(--ink-100);
        }

        [data-theme="strike48-light"] ::selection {
            background: rgba(57, 120, 213, 0.2);
            color: var(--ink-100);
        }

        *:focus {
            outline: none;
        }

        *:focus-visible {
            outline: 2px solid var(--brand-500);
            outline-offset: 1px;
            border-radius: 2px;
        }

        input:focus-visible,
        textarea:focus-visible,
        select:focus-visible {
            outline: none;
            border-color: var(--brand-500);
        }

        /* ============================================
           STRIKE48 COMPONENT CLASSES
           ============================================ */

        /* --- Chips --- */
        .chip {
            display: inline-flex;
            align-items: center;
            gap: 4px;
            height: 18px;
            padding: 0 6px;
            border-radius: 2px;
            font-family: var(--font-mono);
            font-size: 10.5px;
            font-weight: 600;
            letter-spacing: 0.04em;
            text-transform: uppercase;
            line-height: 1;
            border: 1px solid currentColor;
        }

        .chip-dot {
            width: 5px;
            height: 5px;
            border-radius: 999px;
            background: currentColor;
            box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.05);
        }

        .chip-solid {
            color: #fff;
            border-color: transparent;
        }

        /* --- Keyboard shortcut chips --- */
        .kbd-mini {
            display: inline-flex;
            align-items: center;
            justify-content: center;
            min-width: 16px;
            height: 16px;
            padding: 0 4px;
            font-family: var(--font-mono);
            font-size: 10px;
            line-height: 1;
            font-weight: 600;
            color: var(--ink-300);
            background: var(--ink-750);
            border: 1px solid var(--ink-650);
            border-bottom-width: 2px;
            border-radius: 3px;
        }

        [data-theme="strike48-light"] .kbd-mini {
            color: #4B5563;
            background: #FFFFFF;
            border-color: #D1D5DB;
        }

        /* --- Status badges --- */
        .s48-status-critical { background-color: rgba(239, 68, 68, 0.12); color: #ef4444; }
        .s48-status-high { background-color: rgba(249, 115, 22, 0.12); color: #f97316; }
        .s48-status-medium { background-color: rgba(59, 130, 246, 0.12); color: #3b82f6; }
        .s48-status-low { background-color: rgba(100, 116, 139, 0.12); color: #64748b; }
        .s48-status-resolved { background-color: rgba(16, 185, 129, 0.12); color: #10b981; }
        .s48-status-in-progress { background-color: rgba(234, 179, 8, 0.12); color: #eab308; }
        .s48-status-waiting { background-color: rgba(168, 85, 247, 0.12); color: #a855f7; }

        /* --- Row states --- */
        .row-selected {
            background-color: rgba(57, 120, 213, 0.10) !important;
            box-shadow: inset 2px 0 0 var(--brand-500);
        }

        .row-hover:hover {
            background-color: rgba(255, 255, 255, 0.025);
        }

        [data-theme="strike48-light"] .row-selected {
            background-color: rgba(57, 120, 213, 0.08) !important;
        }

        [data-theme="strike48-light"] .row-hover:hover {
            background-color: rgba(0, 0, 0, 0.025);
        }

        /* --- Hairline dividers --- */
        .hairline {
            border: 0;
            border-top: 1px solid var(--ink-700);
        }

        [data-theme="strike48-light"] .hairline {
            border-top-color: #E5E7EB;
        }

        /* --- Live indicators --- */
        @keyframes pulseDot {
            0%, 100% { opacity: 1; transform: scale(1); }
            50% { opacity: 0.45; transform: scale(0.85); }
        }

        .live-dot {
            animation: pulseDot 1.4s ease-in-out infinite;
        }

        @keyframes rowHighlight {
            0% { background-color: rgba(57, 120, 213, 0.18); }
            100% { background-color: transparent; }
        }

        .row-new {
            animation: rowHighlight 2.4s ease-out;
        }

        /* --- Typography helpers --- */
        .s48-display {
            font-family: var(--font-sans);
            font-size: 1.5rem;
            font-weight: 700;
            line-height: 1.2;
        }

        .s48-heading {
            font-family: var(--font-sans);
            font-size: 1.125rem;
            font-weight: 600;
            line-height: 1.3;
        }

        .s48-eyebrow {
            font-family: var(--font-mono);
            font-size: 10px;
            font-weight: 600;
            text-transform: uppercase;
            letter-spacing: 0.1em;
            color: var(--ink-400);
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
        var useDark = stored ? stored === 'dark' : prefersDark;

        if (useDark) {
            document.documentElement.setAttribute('data-theme', 'strike48');
        } else {
            document.documentElement.setAttribute('data-theme', 'strike48-light');
        }
    })();
    "#
}
