// 设备按键图标生成。用 offscreen canvas 画好看的字母徽章 / 箭头 / 对勾，
// 输出 PNG base64（data URL）。无新依赖；设备会自行 re-encode，故这里用 2x 分辨率保证清晰。
//
// AKP153 按键物理分辨率 85×85（见 opendecknew imagespec.json），这里按 2x 渲染。

const SIZE = 170; // 2x of 85
const RADIUS = 28;

type IconColors = {
  /** 背景渐变起色。 */
  bg: string;
  /** 背景渐变止色（不传则纯色）。 */
  bg2?: string;
  /** 前景（文字/图形）颜色。 */
  fg: string;
  /** 可选描边色（用于选中态高亮边框）。 */
  border?: string;
};

/** 简单缓存，避免每帧重复绘制相同图标。 */
const cache = new Map<string, string>();

function getCtx(): { canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D } | null {
  if (typeof document === 'undefined') return null;
  const canvas = document.createElement('canvas');
  canvas.width = SIZE;
  canvas.height = SIZE;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;
  return { canvas, ctx };
}

function roundedRectPath(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

function paintBackground(ctx: CanvasRenderingContext2D, colors: IconColors) {
  const pad = 6;
  roundedRectPath(ctx, pad, pad, SIZE - pad * 2, SIZE - pad * 2, RADIUS);
  if (colors.bg2) {
    const grad = ctx.createLinearGradient(0, 0, 0, SIZE);
    grad.addColorStop(0, colors.bg);
    grad.addColorStop(1, colors.bg2);
    ctx.fillStyle = grad;
  } else {
    ctx.fillStyle = colors.bg;
  }
  ctx.fill();
  if (colors.border) {
    ctx.lineWidth = 8;
    ctx.strokeStyle = colors.border;
    roundedRectPath(ctx, pad + 4, pad + 4, SIZE - (pad + 4) * 2, SIZE - (pad + 4) * 2, RADIUS - 4);
    ctx.stroke();
  }
}

function output(canvas: HTMLCanvasElement, key: string): string {
  const url = canvas.toDataURL('image/png');
  cache.set(key, url);
  return url;
}

/** 字母徽章（A–G）。selected 时绿底白字 + 高亮边。 */
export function letterIcon(letter: string, selected: boolean): string {
  const key = `letter:${letter}:${selected ? 1 : 0}`;
  const cached = cache.get(key);
  if (cached) return cached;
  const env = getCtx();
  if (!env) return '';
  const { canvas, ctx } = env;
  const colors: IconColors = selected
    ? { bg: '#34d27b', bg2: '#1f9d59', fg: '#ffffff', border: '#9bf0c4' }
    : { bg: '#2b3346', bg2: '#1b2130', fg: '#e8edf6' };
  paintBackground(ctx, colors);
  ctx.fillStyle = colors.fg;
  ctx.font = 'bold 96px system-ui, "Segoe UI", Arial, sans-serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(letter.toUpperCase(), SIZE / 2, SIZE / 2 + 6);
  return output(canvas, key);
}

/** 箭头（chevron）。dir: 'left' | 'right'。enabled=false 时灰显。 */
export function arrowIcon(dir: 'left' | 'right', enabled: boolean): string {
  const key = `arrow:${dir}:${enabled ? 1 : 0}`;
  const cached = cache.get(key);
  if (cached) return cached;
  const env = getCtx();
  if (!env) return '';
  const { canvas, ctx } = env;
  const colors: IconColors = enabled
    ? { bg: '#1e6fff', bg2: '#1657c8', fg: '#ffffff' }
    : { bg: '#2a2f3a', fg: '#5a6172' };
  paintBackground(ctx, colors);
  ctx.strokeStyle = colors.fg;
  ctx.lineWidth = 16;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  const cx = SIZE / 2;
  const cy = SIZE / 2;
  const half = 30;
  ctx.beginPath();
  if (dir === 'left') {
    ctx.moveTo(cx + half * 0.6, cy - half);
    ctx.lineTo(cx - half * 0.6, cy);
    ctx.lineTo(cx + half * 0.6, cy + half);
  } else {
    ctx.moveTo(cx - half * 0.6, cy - half);
    ctx.lineTo(cx + half * 0.6, cy);
    ctx.lineTo(cx - half * 0.6, cy + half);
  }
  ctx.stroke();
  return output(canvas, key);
}

/** 提交（对勾）。 */
export function submitIcon(): string {
  const key = 'submit';
  const cached = cache.get(key);
  if (cached) return cached;
  const env = getCtx();
  if (!env) return '';
  const { canvas, ctx } = env;
  paintBackground(ctx, { bg: '#f0a52a', bg2: '#d4870f', fg: '#ffffff' });
  ctx.strokeStyle = '#ffffff';
  ctx.lineWidth = 18;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  ctx.beginPath();
  ctx.moveTo(SIZE * 0.3, SIZE * 0.52);
  ctx.lineTo(SIZE * 0.45, SIZE * 0.68);
  ctx.lineTo(SIZE * 0.72, SIZE * 0.34);
  ctx.stroke();
  return output(canvas, key);
}
