const color = (name) => `color-mix(in oklch, var(--${name}) calc(<alpha-value> * 100%), transparent)`

/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        border: color("border"),
        input: color("input"),
        ring: color("ring"),
        background: color("background"),
        foreground: color("foreground"),
        primary: {
          DEFAULT: color("primary"),
          foreground: color("primary-foreground"),
        },
        secondary: {
          DEFAULT: color("secondary"),
          foreground: color("secondary-foreground"),
        },
        destructive: {
          DEFAULT: color("destructive"),
          foreground: color("destructive-foreground"),
        },
        muted: {
          DEFAULT: color("muted"),
          foreground: color("muted-foreground"),
        },
        accent: {
          DEFAULT: color("accent"),
          foreground: color("accent-foreground"),
        },
        card: {
          DEFAULT: color("card"),
          foreground: color("card-foreground"),
        },
        popover: {
          DEFAULT: color("popover"),
          foreground: color("popover-foreground"),
        },
        sidebar: {
          DEFAULT: color("sidebar"),
          foreground: color("sidebar-foreground"),
          primary: color("sidebar-primary"),
          "primary-foreground": color("sidebar-primary-foreground"),
          accent: color("sidebar-accent"),
          "accent-foreground": color("sidebar-accent-foreground"),
          border: color("sidebar-border"),
          ring: color("sidebar-ring"),
        },
        chart: {
          1: color("chart-1"),
          2: color("chart-2"),
          3: color("chart-3"),
          4: color("chart-4"),
          5: color("chart-5"),
        },
      },
      fontFamily: {
        sans: ["var(--font-sans)"],
        serif: ["var(--font-serif)"],
        mono: ["var(--font-mono)"],
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      boxShadow: {
        "2xs": "var(--shadow-2xs)",
        xs: "var(--shadow-xs)",
        sm: "var(--shadow-sm)",
        DEFAULT: "var(--shadow)",
        md: "var(--shadow-md)",
        lg: "var(--shadow-lg)",
        xl: "var(--shadow-xl)",
        "2xl": "var(--shadow-2xl)",
      },
    },
  },
  plugins: [],
}
