import type { Config } from 'tailwindcss'

export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      fontFamily: {
        mono:    ['"IBM Plex Mono"', 'monospace'],
        display: ['"Syne"', 'sans-serif'],
        sans:    ['"IBM Plex Sans"', 'sans-serif'],
      },
      colors: {
        caiman: {
          bg:       '#070d07',
          bg2:      '#0a120a',
          bg3:      '#0d1a0d',
          bg4:      '#0a150a',
          border:   '#1a3d1a',
          border2:  '#2e7d32',
          green:    '#4caf50',
          bright:   '#76ff03',
          dim:      '#4a7c4a',
          text:     '#c8e6c9',
          muted:    '#6a9b6a',
          amber:    '#ffb300',
          red:      '#ef5350',
          blue:     '#42a5f5',
          purple:   '#ab47bc',
        },
      },
      animation: {
        'heartbeat':  'heartbeat 2s ease-in-out infinite',
        'pulse-fast': 'pulse 1s ease-in-out infinite',
        'slide-in':   'slideIn 0.25s ease-out',
        'fade-in':    'fadeIn 0.15s ease-out',
        'scan':       'scan 3s linear infinite',
      },
      keyframes: {
        heartbeat: {
          '0%, 100%': { transform: 'scale(1)', opacity: '1' },
          '40%':      { transform: 'scale(0.7)', opacity: '0.5' },
          '60%':      { transform: 'scale(1.1)', opacity: '1' },
        },
        slideIn: {
          from: { transform: 'translateX(100%)', opacity: '0' },
          to:   { transform: 'translateX(0)',    opacity: '1' },
        },
        fadeIn: {
          from: { opacity: '0' },
          to:   { opacity: '1' },
        },
        scan: {
          '0%':   { transform: 'translateY(-100%)' },
          '100%': { transform: 'translateY(100vh)' },
        },
      },
      boxShadow: {
        'caiman':      '0 0 0 1px #1a3d1a',
        'caiman-glow': '0 0 12px rgba(118, 255, 3, 0.15)',
        'panel':       '0 4px 24px rgba(0,0,0,0.4)',
      },
    },
  },
  plugins: [],
} satisfies Config
