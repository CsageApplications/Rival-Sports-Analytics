module.exports = {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        brand: {
          bg:      '#060b14',
          bg2:     '#080e18',
          bg3:     '#0c1420',
          bg4:     '#101a28',
          border:  'rgba(255,255,255,0.07)',
          borderCyan: 'rgba(0,212,255,0.16)',
          cyan:    '#00d4ff',
          cyanDim: 'rgba(0,212,255,0.10)',
          violet:  '#7c3aed',
          violetDim: 'rgba(124,58,237,0.10)',
          text:    '#cfe6f2',
          textDim: '#7aa3ba',
          textMuted: '#3a5566',
          green:   '#22d3a5',
          amber:   '#f59e0b',
          red:     '#ef4444',
        },
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', 'sans-serif'],
        mono: ['"JetBrains Mono"', 'ui-monospace', 'monospace'],
      },
    },
  },
  plugins: [],
}
