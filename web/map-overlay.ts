import type { MapFrame, Vector2 } from './types.js';
// Shared SVG projection for overlays aligned with Bevy's AutoMin(2000, 1200) camera.
export function mapProjection(svg: SVGSVGElement, map: MapFrame) {
  const box = svg.getBoundingClientRect();
  const width = Math.max(box.width, 1), height = Math.max(box.height, 1);
  const halfWidth = Math.max(1000, 600 * width / height);
  const unitsPerPixel = halfWidth * 2 / width;
  const metresPerPixel = unitsPerPixel * map.metres_per_unit;
  svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
  const project = (p: Vector2): [number, number] => [width / 2 + (p.x - map.center.x) / metresPerPixel,
    height / 2 - (p.y - map.center.y) / metresPerPixel];
  return { box, width, height, unitsPerPixel, metresPerPixel, project,
    unproject: (x: number, y: number) => ({ x: map.center.x + (x - box.left - width / 2) * metresPerPixel,
      y: map.center.y - (y - box.top - height / 2) * metresPerPixel }),
    path: (points: Vector2[]) => points.map((p, i) => `${i ? 'L' : 'M'}${project(p).join(' ')}`).join(' '),
  };
}

export type Projection = ReturnType<typeof mapProjection>;
