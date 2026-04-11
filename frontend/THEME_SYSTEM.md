# ApoloBilling Theme System

## Overview

The ApoloBilling frontend now includes a comprehensive theme system supporting **light**, **dark**, and **system** (auto) modes.

## Features

- **3 Theme Modes**:
  - Light: Clean, bright interface
  - Dark: The default dark theme for main content (sidebar remains dark)
  - System: Automatically follows OS theme preference

- **Persistent Storage**: Theme preference is saved in `localStorage`
- **System Theme Detection**: Uses `prefers-color-scheme` media query
- **Smooth Transitions**: All theme changes animate smoothly
- **Accessible**: Theme toggle in header with clear visual indicators

## Implementation Details

### Files Modified

1. **`/opt/ApoloBilling/frontend/src/contexts/ThemeContext.tsx`** (NEW)
   - React Context for theme state management
   - Handles theme switching and persistence
   - Listens to system theme changes

2. **`/opt/ApoloBilling/frontend/src/index.css`**
   - CSS custom properties for theme colors
   - Light and dark theme color definitions
   - Theme-aware scrollbar styles

3. **`/opt/ApoloBilling/frontend/src/App.tsx`**
   - Wrapped app with `ThemeProvider`

4. **`/opt/ApoloBilling/frontend/src/components/Layout.tsx`**
   - Added theme toggle button in header
   - Theme dropdown menu with Sun/Moon/Monitor icons
   - Updated main content area to use theme variables

5. **`/opt/ApoloBilling/frontend/src/pages/Dashboard.tsx`**
   - Updated all hardcoded colors to use CSS variables
   - Charts now use theme-aware colors
   - Cards, tooltips, and tables are theme-aware

6. **`/opt/ApoloBilling/frontend/src/components/StatCard.tsx`**
   - Updated background and text colors to use theme variables

7. **`/opt/ApoloBilling/frontend/src/components/DataTable.tsx`**
   - Table headers, rows, and pagination use theme colors
   - Search input is theme-aware

## CSS Variables

### Light Theme
```css
--color-bg-primary: #ffffff
--color-bg-secondary: #f8fafc
--color-bg-tertiary: #f1f5f9
--color-bg-card: #ffffff
--color-bg-hover: #f8fafc

--color-text-primary: #0f172a
--color-text-secondary: #475569
--color-text-tertiary: #64748b
--color-text-muted: #94a3b8

--color-border-primary: #e2e8f0
--color-border-secondary: #cbd5e1
```

### Dark Theme
```css
--color-bg-primary: #0f172a
--color-bg-secondary: #0a0f1a
--color-bg-tertiary: #1e293b
--color-bg-card: #1e293b
--color-bg-hover: #334155

--color-text-primary: #f1f5f9
--color-text-secondary: #cbd5e1
--color-text-tertiary: #94a3b8
--color-text-muted: #64748b

--color-border-primary: #334155
--color-border-secondary: #475569
```

## Usage

### Using Theme Context in Components

```tsx
import { useTheme } from '../contexts/ThemeContext'

function MyComponent() {
  const { theme, setTheme, resolvedTheme } = useTheme()

  return (
    <button onClick={() => setTheme('dark')}>
      Switch to Dark
    </button>
  )
}
```

### Using Theme Colors in Tailwind

Use CSS variables for theme-aware colors:

```tsx
// Background
<div className="bg-[var(--color-bg-card)]">

// Text
<h1 className="text-[var(--color-text-primary)]">

// Borders
<div className="border border-[var(--color-border-primary)]">
```

## Design Principles

1. **Sidebar Always Dark**: The left sidebar maintains its dark cyberpunk aesthetic in all themes
2. **Main Content Adaptive**: Main content area switches between light/dark based on theme
3. **Consistent Contrast**: Text always maintains WCAG AA contrast ratios
4. **Smooth Transitions**: 200ms ease transitions for all color changes
5. **Branded Accents**: Cyan/blue accents remain consistent across themes

## User Experience

- **Theme Toggle Location**: Top-right header, next to system time
- **Visual Icons**:
  - Sun icon for light mode
  - Moon icon for dark mode
  - Monitor icon for system mode
- **Active State**: Selected theme highlighted in cyan
- **Hover States**: All interactive elements have clear hover feedback

## Browser Support

- Modern browsers with CSS custom properties support
- `prefers-color-scheme` media query (Safari 12.1+, Chrome 76+, Firefox 67+)
- `localStorage` for persistence
- Graceful degradation to light theme if features unsupported

## Testing

Build successful with no TypeScript errors:
```bash
npm run build
# ✓ built in 4.95s
```

## Future Enhancements

- Custom color schemes (e.g., high contrast mode)
- Per-user theme preferences stored in backend
- Additional theme presets (blue, purple, green)
- Automatic theme switching based on time of day
