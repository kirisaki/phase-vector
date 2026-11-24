import { toCanvas } from 'html-to-image';

const MAX_BYTES = 2 * 1024 * 1024;
// Copy WebGL within a render frame: its default framebuffer need not survive
// until html-to-image asynchronously reaches the canvas during DOM cloning.
async function freezeCanvas(canvas: HTMLCanvasElement): Promise<HTMLCanvasElement> {
  for (let attempt = 0; attempt < 8; attempt++) {
    const copy = await new Promise<HTMLCanvasElement>((resolve, reject) => {
      requestAnimationFrame(() => {
        try {
          const target = document.createElement('canvas');
          target.width = canvas.width; target.height = canvas.height;
          const context = target.getContext('2d', { willReadFrequently: true });
          if (!context) throw new Error('Canvas capture unavailable');
          context.drawImage(canvas, 0, 0);
          resolve(target);
        } catch (error) { reject(error); }
      });
    });
    const probe = document.createElement('canvas'); probe.width = probe.height = 48;
    const context = probe.getContext('2d')!; context.drawImage(copy, 0, 0, 48, 48);
    const pixels = context.getImageData(0, 0, 48, 48).data;
    if (pixels.some((value, i) => i % 4 === 3 && value > 0)) return copy;
  }
  throw new Error('The WebGL frame was unavailable');
}
function freezeSvgTrees(): () => void {
  // html-to-image deep-clones SVG trees without visiting their descendants.
  // Supply frozen clones with resolved paint/text styles, including dynamic
  // markers that the normal UI refresh can replace while DOM capture awaits.
  const properties = ['fill', 'fill-opacity', 'fill-rule', 'stroke', 'stroke-width',
    'stroke-opacity', 'stroke-dasharray', 'stroke-dashoffset', 'stroke-linecap',
    'stroke-linejoin', 'opacity', 'font-family', 'font-size', 'font-weight',
    'font-style', 'letter-spacing', 'text-anchor', 'dominant-baseline', 'paint-order',
    'visibility', 'display', 'clip-path'];
  const saved = [...document.querySelectorAll<SVGSVGElement>('svg')].map(svg => {
    const frozen = svg.cloneNode(true) as SVGSVGElement;
    const sources = [svg, ...svg.querySelectorAll<SVGElement>('*')];
    const targets = [frozen, ...frozen.querySelectorAll<SVGElement>('*')];
    sources.forEach((source, i) => {
      const computed = getComputedStyle(source);
      for (const property of properties) {
        let value = computed.getPropertyValue(property);
        if (property === 'clip-path') value = value.replace(/url\(["']?[^)]*#([^"')]+)["']?\)/g, 'url("#$1")');
        targets[i].style.setProperty(property, value);
      }
    });
    const original = Object.getOwnPropertyDescriptor(svg, 'cloneNode');
    Object.defineProperty(svg, 'cloneNode', { configurable: true, value: (deep: boolean) => frozen.cloneNode(deep) });
    return { svg, original };
  });
  return () => { for (const { svg, original } of saved) {
    if (original) Object.defineProperty(svg, 'cloneNode', original); else delete (svg as Partial<SVGSVGElement>).cloneNode;
  } };
}
export async function captureApplication(): Promise<Blob> {
  const canvas = document.querySelector<HTMLCanvasElement>('#tactical-map');
  if (!canvas?.width || !canvas.height) throw new Error('The game canvas is unavailable');
  const frozen = await freezeCanvas(canvas);
  const dataUrl = frozen.toDataURL('image/png');
  const original = Object.getOwnPropertyDescriptor(canvas, 'toDataURL');
  // Only override serialization on this instance; the renderer and its context
  // continue unchanged. Always restore it, including failed DOM captures.
  Object.defineProperty(canvas, 'toDataURL', { configurable: true, value: () => dataUrl });
  const restoreSvg = freezeSvgTrees();
  try {
    const result = await toCanvas(document.body, {
      width: innerWidth, height: innerHeight, pixelRatio: Math.min(1, 1600 / Math.max(innerWidth, innerHeight)),
      backgroundColor: '#070f16', skipFonts: true,
      style: { margin: '0', top: '0', left: '0', transform: 'none' },
      filter: node => !['console-panel', 'error', 'score-victory-detail', 'feedback-dialog'].includes(node.id)
        && !['SCRIPT', 'IFRAME'].includes(node.tagName),
    });
    const blob = await new Promise<Blob>((resolve, reject) => result.toBlob(value => value ? resolve(value) : reject(new Error('PNG encoding failed')), 'image/png'));
    if (blob.size > MAX_BYTES) throw new Error('Screenshot exceeds the 2 MiB attachment limit');
    return blob;
  } finally {
    restoreSvg();
    if (original) Object.defineProperty(canvas, 'toDataURL', original);
    else delete (canvas as Partial<HTMLCanvasElement>).toDataURL;
  }
}
