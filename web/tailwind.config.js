/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        side: '#f7f8fa',
        topbar: '#f2f3f5',
        accent: { DEFAULT: '#1677ff', soft: '#e8f1fe', deep: '#0958d9' },
        ink: { DEFAULT: '#1f2329', 2: '#4e5969', 3: '#86909c' },
        line: '#e5e6eb',
        ok: '#00b42a',
        warn: '#ff7d00',
        danger: '#f53f3f',
      },
      borderRadius: { pane: '12px' },
      boxShadow: {
        pane: '0 1px 2px rgba(29,33,41,.03), 0 6px 20px rgba(29,33,41,.05)',
        chip: '0 1px 3px rgba(29,33,41,.08)',
        dialog: '0 16px 48px rgba(29,33,41,.20), 0 4px 12px rgba(29,33,41,.10)',
      },
      keyframes: {
        paneIn: { from: { opacity: '0', transform: 'translateY(8px)' }, to: { opacity: '1', transform: 'none' } },
        popIn: { from: { opacity: '0', transform: 'scale(.96)' }, to: { opacity: '1', transform: 'scale(1)' } },
        dlgIn: { from: { opacity: '0', transform: 'translateY(10px) scale(.97)' }, to: { opacity: '1', transform: 'none' } },
        fadeIn: { from: { opacity: '0' }, to: { opacity: '1' } },
        breathe: { '0%,100%': { boxShadow: '0 0 0 0 rgba(22,119,255,.25)' }, '50%': { boxShadow: '0 0 0 6px rgba(22,119,255,0)' } },
        ripple: { from: { transform: 'scale(.4)', opacity: '1' }, to: { transform: 'scale(4.5)', opacity: '0' } },
        scanline: { to: { transform: 'translateY(135vh)' } },
        blink: { '0%,100%': { opacity: '1' }, '50%': { opacity: '.35' } },
      },
      animation: {
        paneIn: 'paneIn .28s ease both',
        popIn: 'popIn .18s ease both',
        dlgIn: 'dlgIn .18s cubic-bezier(.2,.8,.3,1) both',
        fadeIn: 'fadeIn .15s ease both',
        breathe: 'breathe 2.4s ease-in-out infinite',
        ripple: 'ripple .7s ease-out forwards',
        scanline: 'scanline 1.6s linear forwards',
        blink: 'blink 1s ease-in-out infinite',
      },
    },
  },
  plugins: [],
};