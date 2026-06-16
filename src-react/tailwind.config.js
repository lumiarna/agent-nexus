/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{ts,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          "Plus Jakarta Sans",
          "-apple-system",
          "BlinkMacSystemFont",
          "sans-serif",
        ],
        mono: ["ui-monospace", "Menlo", "monospace"],
      },
      // High-frequency structural + status colors from prototype/nexus-data.js.
      // Long-tail one-off shades stay as arbitrary values (`[#hex]`) for exact
      // fidelity. Runtime-computed colors live in lib/tokens.ts.
      colors: {
        nexus: {
          bg: "#f3ece3",
          card: "#fcf9f4",
          sand: "#f6efe5",
          sand2: "#f8f3ea",
          panel: "#f0e8db",
          titlebar: "#ece2d5",
          ink: "#2a2520",
          body: "#3a302a",
          accent: "#9d7a64",
          "accent-hover": "#8c6b56",
          border: "#efe6da",
          border2: "#e7ddce",
          good: "#8a9a5b",
          warn: "#c2913f",
          crit: "#b55440",
        },
      },
      keyframes: {
        "ann-fade": { from: { opacity: "0" }, to: { opacity: "1" } },
        "ann-pulse": {
          "0%,100%": { opacity: "1" },
          "50%": { opacity: "0.4" },
        },
      },
      animation: {
        "ann-fade": "ann-fade .14s ease-out",
        "ann-pulse": "ann-pulse 1s ease-in-out infinite",
      },
    },
  },
  plugins: [],
};
