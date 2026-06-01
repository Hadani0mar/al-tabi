/* global React */
// Lucide-style SVG icons used throughout the al-tabi UI kit.
// Stroke 1.8, currentColor — inherits text color.

const I = ({ children, size = 18 }) =>
  React.createElement(
    'svg',
    {
      viewBox: '0 0 24 24',
      width: size,
      height: size,
      fill: 'none',
      stroke: 'currentColor',
      strokeWidth: 1.8,
      strokeLinecap: 'round',
      strokeLinejoin: 'round',
      'aria-hidden': true,
    },
    children
  );
const p = (d, key) => React.createElement('path', { d, key });
const c = (cx, cy, r, key) => React.createElement('circle', { cx, cy, r, key });
const r = (x, y, w, h, rx, key) => React.createElement('rect', { x, y, width: w, height: h, rx, key });
const pl = (points, key) => React.createElement('polyline', { points, key });

// Each icon is a React component: accepts { size = 18 } props.
const Icon = {
  Grid:     ({ size }) => <I size={size}>{r(3,3,7,7,1,'a')}{r(14,3,7,7,1,'b')}{r(14,14,7,7,1,'c')}{r(3,14,7,7,1,'d')}</I>,
  Search:   ({ size }) => <I size={size}>{c(11,11,7,'a')}{p('m20 20-3.5-3.5','b')}</I>,
  Bell:     ({ size }) => <I size={size}>{p('M6 8a6 6 0 1 1 12 0c0 7 3 9 3 9H3s3-2 3-9','a')}{p('M10 21a2 2 0 0 0 4 0','b')}</I>,
  Sparkles: ({ size }) => <I size={size}>{p('M12 3v3M12 18v3M5 12H2M22 12h-3M6 6l2 2M16 16l2 2M6 18l2-2M16 8l2-2','a')}{c(12,12,3.6,'b')}</I>,
  Bookmark: ({ size }) => <I size={size}>{p('M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z','a')}</I>,
  Settings: ({ size }) => <I size={size}>{c(12,12,3,'a')}{p('M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z','b')}</I>,
  Calendar: ({ size }) => <I size={size}>{r(3,4,18,18,2,'a')}{p('M16 2v4M8 2v4M3 10h18','b')}</I>,
  Download: ({ size }) => <I size={size}>{p('M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3','a')}</I>,
  Save:     ({ size }) => <I size={size}>{p('M21 19V5a2 2 0 0 0-2-2H8l-5 5v11a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2zM17 21v-8H7v8M7 3v5h8','a')}</I>,
  File:     ({ size }) => <I size={size}>{p('M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z','a')}{p('M14 2v6h6M16 13H8M16 17H8M10 9H8','b')}</I>,
  TrendUp:  ({ size }) => <I size={size}>{p('M3 3v18h18M7 17l4-4 4 4 6-6','a')}</I>,
  Menu:     ({ size }) => <I size={size}>{p('M3 6h18M3 12h18M3 18h18','a')}</I>,
  Info:     ({ size }) => <I size={size}>{c(12,12,10,'a')}{p('M12 16v-4M12 8h.01','b')}</I>,
  Trash:    ({ size }) => <I size={size}>{p('M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6','a')}</I>,
  Plus:     ({ size }) => <I size={size}>{p('M12 5v14M5 12h14','a')}</I>,
  Check:    ({ size }) => <I size={size}>{pl('5 12 10 17 19 7','a')}</I>,
  X:        ({ size }) => <I size={size}>{p('M18 6 6 18M6 6l12 12','a')}</I>,
  Send:     ({ size }) => <I size={size}>{p('m22 2-7 20-4-9-9-4Z','a')}{p('M22 2 11 13','b')}</I>,
  Play:     ({ size }) => <I size={size}>{p('M5 3l14 9-14 9V3z','a')}</I>,
  Pause:    ({ size }) => <I size={size}>{r(6,4,4,16,1,'a')}{r(14,4,4,16,1,'b')}</I>,
  Database: ({ size }) => <I size={size}>{p('M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3-3.6 3-8 3-8-1.3-8-3z','a')}{p('M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6','b')}{p('M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3','c')}</I>,
  Telegram: ({ size }) => <I size={size}>{p('m22 2-7 20-4-9-9-4Z','a')}{p('M22 2 11 13','b')}</I>,
  Sun:      ({ size }) => <I size={size}>{c(12,12,4,'a')}{p('M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41','b')}</I>,
  Moon:     ({ size }) => <I size={size}>{p('M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z','a')}</I>,
  Eye:      ({ size }) => <I size={size}>{p('M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z','a')}{c(12,12,3,'b')}</I>,
  EyeOff:   ({ size }) => <I size={size}>{p('M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19M1 1l22 22M14.12 14.12a3 3 0 1 1-4.24-4.24','a')}</I>,
  Edit:     ({ size }) => <I size={size}>{p('M12 20h9M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4z','a')}</I>,
  Filter:   ({ size }) => <I size={size}>{p('M22 3H2l8 9.46V19l4 2v-8.54z','a')}</I>,
  ChevronRight: ({ size }) => <I size={size}>{p('m9 18 6-6-6-6','a')}</I>,
  Bot:      ({ size }) => <I size={size}>{r(3,11,18,10,2,'a')}{c(8,16,1,'b')}{c(16,16,1,'c')}{p('M12 7v4M9 4h6','d')}</I>,
  Spark:    ({ size }) => <I size={size}>{p('M12 3l1.9 5.1 5.1 1.9-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9z','a')}</I>,
};

window.Icon = Icon;
