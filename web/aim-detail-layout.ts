import type { Vector2 } from './types.js';
import { AIM_DETAIL } from './ui-settings.js';

interface Size { width: number; height: number }
type Side = 'above' | 'below' | 'left' | 'right';
export interface DetailPlacement extends Vector2 { side: Side }

// All positions are in surface-local pixels; this calculation does not read the DOM.
export function placeAimDetail(
  bounds: Size, panel: Size, anchor: Vector2, aim: Vector2,
  pointer?: Vector2, previousSide?: Side,
): DetailPlacement {
  const { width: w, height: h } = panel;
  const edge = AIM_DETAIL.edgePaddingPixels, gap = AIM_DETAIL.handleGapPixels;
  const avoid = [anchor, aim, ...(pointer ? [pointer] : [])];
  const positions: DetailPlacement[] = [
    { side: 'above', x: anchor.x - w / 2, y: anchor.y - gap - h },
    { side: 'below', x: anchor.x - w / 2, y: anchor.y + gap },
    { side: 'left', x: anchor.x - gap - w, y: anchor.y - h / 2 },
    { side: 'right', x: anchor.x + gap, y: anchor.y - h / 2 },
  ];
  const candidates = positions.map(candidate => {
    const x = Math.max(edge, Math.min(bounds.width - w - edge, candidate.x));
    const y = Math.max(edge, Math.min(bounds.height - h - edge, candidate.y));
    const distance = (p: Vector2) => Math.hypot(Math.max(x - p.x, 0, p.x - x - w), Math.max(y - p.y, 0, p.y - y - h));
    const covered = avoid.filter(p => distance(p) === 0).length;
    const overlap = avoid.reduce((sum, p) => sum + Math.max(0, AIM_DETAIL.pointerClearancePixels - distance(p)), 0);
    return { side: candidate.side, x, y, covered, overlap, distance: distance(anchor), stable: candidate.side === previousSide };
  });
  // Avoid controls first, then stay near the handle. Prefer the previous side on ties.
  candidates.sort((a, b) => a.covered - b.covered || a.overlap - b.overlap || a.distance - b.distance || Number(b.stable) - Number(a.stable));
  const { side, x, y } = candidates[0];
  return { side, x, y };
}
