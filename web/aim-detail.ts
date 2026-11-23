import { element, query, required } from './dom.js';
import type { Projection } from './map-overlay.js';
import type { ArrivalEstimate, Solution, Vector2 } from './types.js';
import { AIM_DETAIL } from './ui-settings.js';

interface ObservedCourse { position: Vector2; velocity: Vector2; horizonSeconds: number }
const shipSymbol = 'M8 0L-6 5L-3 0L-6 -5Z';

// A passive local view of the existing shot preview, never a second simulation.
export function createAimDetail() {
  const panel = element('aim-detail'), svg = element('aim-detail-map');
  panel.style.setProperty('--detail-edge', `${AIM_DETAIL.edgePaddingPixels}px`);
  const offset = element('aim-detail-offset'), note = element('aim-detail-note');
  const shape = (name: string) => query<SVGPathElement>(`.detail-${name}`, svg);
  const grid = shape('grid'), shot = shape('shot'), gap = shape('offset');
  const enemy = shape('enemy'), crosshair = shape('crosshair'), scale = shape('scale');
  const route = shape('route'), current = shape('current');
  const currentLabel = query<SVGTextElement>('.detail-current-label', svg);
  const grip = query<SVGGElement>('.grip-control', element('aim-grip'));
  const surface = element('mission-overlay');
  const scaleLabel = query<SVGTextElement>('.detail-scale-label', svg);
  const clip = query<SVGRectElement>('#aim-detail-clip rect', svg);
  const km = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 });
  const scaleKm = new Intl.NumberFormat('en-US', { maximumSignificantDigits: 3 });
  let metresPerPixel = 1;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let lastPointer: Vector2 | undefined;
  let placement = '';

  function positionPanel(pointer?: Vector2) {
    if (pointer) lastPointer = pointer;
    const bounds = surface.getBoundingClientRect(), panelBox = panel.getBoundingClientRect();
    const center = (node: SVGGElement) => {
      const p = new DOMPoint(0, 0).matrixTransform(required(node.getScreenCTM()));
      return { x: p.x - bounds.left, y: p.y - bounds.top };
    };
    const anchor = center(grip), aim = center(element('aim-handle'));
    const { width: w, height: h } = panelBox;
    const edge = AIM_DETAIL.edgePaddingPixels, gap = AIM_DETAIL.handleGapPixels;
    const avoid = [anchor, aim, ...(lastPointer ? [{ x: lastPointer.x - bounds.left, y: lastPointer.y - bounds.top }] : [])];
    const candidates = [
      { side: 'above', x: anchor.x - w / 2, y: anchor.y - gap - h },
      { side: 'below', x: anchor.x - w / 2, y: anchor.y + gap },
      { side: 'left', x: anchor.x - gap - w, y: anchor.y - h / 2 },
      { side: 'right', x: anchor.x + gap, y: anchor.y - h / 2 },
    ].map(candidate => {
      const x = Math.max(edge, Math.min(bounds.width - w - edge, candidate.x));
      const y = Math.max(edge, Math.min(bounds.height - h - edge, candidate.y));
      const distance = (p: Vector2) => Math.hypot(Math.max(x - p.x, 0, p.x - x - w), Math.max(y - p.y, 0, p.y - y - h));
      const covered = avoid.filter(p => distance(p) === 0).length;
      const overlap = avoid.reduce((sum, p) => sum + Math.max(0, AIM_DETAIL.pointerClearancePixels - distance(p)), 0);
      // Avoid controls first, then stay near the handle. Prefer the previous side on ties.
      return { side: candidate.side, x, y, covered, overlap, distance: distance(anchor), stable: candidate.side === placement };
    });
    candidates.sort((a, b) => a.covered - b.covered || a.overlap - b.overlap || a.distance - b.distance || Number(b.stable) - Number(a.stable));
    const best = candidates[0];
    placement = best.side;
    panel.dataset.placement = placement;
    panel.style.left = `${best.x}px`; panel.style.top = `${best.y}px`;
  }

  function hide() {
    clearTimeout(hideTimer); hideTimer = undefined;
    panel.hidden = true;
    delete panel.dataset.phase;
    lastPointer = undefined; placement = '';
  }

  return {
    hide,
    start(view: Projection) {
      clearTimeout(hideTimer); hideTimer = undefined;
      lastPointer = undefined; placement = '';
      metresPerPixel = view.metresPerPixel / AIM_DETAIL.magnification;
      panel.dataset.phase = 'dragging';
      panel.hidden = false;
    },
    finish(applied: boolean) {
      if (!applied) { hide(); return; }
      panel.dataset.phase = 'lingering';
      clearTimeout(hideTimer);
      hideTimer = setTimeout(hide, AIM_DETAIL.lingerMs);
    },
    render(aim: Vector2 | null | undefined, solution: Solution | null, arrival: ArrivalEstimate | null, course: ObservedCourse, pointer?: Vector2) {
      if (panel.hidden) return;
      if (!aim) { hide(); return; }
      positionPanel(pointer);
      const box = svg.getBoundingClientRect(), w = box.width, h = box.height;
      if (w <= 0 || h <= 0) return;
      svg.setAttribute('viewBox', `0 0 ${w} ${h}`);
      const plotHeight = Math.max(1, h - AIM_DETAIL.scaleAreaPixels);
      clip.setAttribute('width', String(w)); clip.setAttribute('height', String(plotHeight));
      const cx = w / 2, cy = plotHeight / 2;
      grid.setAttribute('d', `M0 ${cy}H${w}M${cx} 0V${plotHeight}`);
      crosshair.setAttribute('d', `M${cx - 9} ${cy}h6m6 0h6M${cx} ${cy - 9}v6m0 6v6`);

      // Only the delivered track extrapolated to now, never the enemy's true state.
      const project = (p: Vector2) => ({ x: cx + (p.x - aim.x) / metresPerPixel, y: cy + (aim.y - p.y) / metresPerPixel });
      const now = project(course.position);
      const future = project({ x: course.position.x + course.velocity.x * course.horizonSeconds,
        y: course.position.y + course.velocity.y * course.horizonSeconds });
      route.setAttribute('d', `M${now.x} ${now.y}L${future.x} ${future.y}`);
      const heading = Math.atan2(-course.velocity.y, course.velocity.x) * 180 / Math.PI;
      const currentVisible = now.x >= 8 && now.x <= w - 8 && now.y >= 8 && now.y <= plotHeight - 8;
      current.setAttribute('d', currentVisible ? shipSymbol : '');
      current.setAttribute('transform', `translate(${now.x} ${now.y}) rotate(${heading})`);
      currentLabel.style.display = currentVisible ? '' : 'none';
      currentLabel.setAttribute('x', String(now.x > cx ? now.x - 12 : now.x + 12));
      currentLabel.setAttribute('text-anchor', now.x > cx ? 'end' : 'start');
      currentLabel.setAttribute('y', String(Math.max(10, now.y - 10)));

      // Draw only the approach to the target; the main preview ends there too.
      if (solution) {
        const dx = (solution.origin.x - aim.x) / metresPerPixel;
        const dy = (aim.y - solution.origin.y) / metresPerPixel;
        const length = Math.hypot(dx, dy), ratio = length > 0 ? Math.min(1, Math.hypot(w, h) / length) : 0;
        shot.setAttribute('d', `M${cx + dx * ratio} ${cy + dy * ratio}L${cx} ${cy}`);
      } else shot.setAttribute('d', '');

      if (arrival) {
        const dx = (arrival.position.x - aim.x) / metresPerPixel;
        const dy = (aim.y - arrival.position.y) / metresPerPixel;
        const edge = AIM_DETAIL.markerInsetPixels;
        const ratio = Math.max(1, Math.abs(dx) / Math.max(1, cx - edge), Math.abs(dy) / Math.max(1, cy - edge));
        const x = cx + dx / ratio, y = cy + dy / ratio;
        const offscreen = ratio > 1;
        gap.setAttribute('d', `M${cx} ${cy}L${x} ${y}`);
        enemy.setAttribute('d', offscreen ? 'M6 0L-4 -5L-4 5Z' : shipSymbol);
        enemy.setAttribute('transform', `translate(${x} ${y}) rotate(${offscreen ? Math.atan2(dy, dx) * 180 / Math.PI : heading})`);
        note.textContent = offscreen ? 'Off view' : 'At impact';
        offset.textContent = `Offset ${km.format(arrival.offsetMetres / 1000)} km`;
      } else {
        enemy.setAttribute('d', ''); gap.setAttribute('d', '');
        note.textContent = 'No solution';
        offset.textContent = 'Shot preview unavailable';
      }

      // A readable 1/2/5 scale bar, in kilometres; marker sizes are symbolic.
      const maximum = w * 0.3 * metresPerPixel;
      const power = 10 ** Math.floor(Math.log10(maximum));
      const distance = (maximum / power >= 5 ? 5 : maximum / power >= 2 ? 2 : 1) * power;
      const pixels = distance / metresPerPixel, y = h - 7;
      scale.setAttribute('d', `M8 ${y - 3}v3h${pixels}v-3`);
      scaleLabel.setAttribute('x', '8'); scaleLabel.setAttribute('y', String(y - 7));
      scaleLabel.textContent = `${scaleKm.format(distance / 1000)} km`;
    },
  };
}
