import { TIMELINE } from './ui-settings.js';

// Reserve space for the imminent command without losing distant firing plans.
export function timeScale(now, end, interval) {
  const boundary = Math.min(end, (Math.floor(now / interval) + 1) * interval);
  const split = boundary < end ? TIMELINE.nearShare : 1;
  return {
    now, end, boundary, split, interval,
    time(position) {
      return position <= split
        ? now + (boundary - now) * position / split
        : boundary + (end - boundary) * (position - split) / (1 - split);
    },
    position(time) {
      if (end <= now) return 0;
      return time <= boundary
        ? split * (time - now) / (boundary - now)
        : split + (1 - split) * (time - boundary) / (end - boundary);
    },
  };
}

// Readiness markers take priority over command boundaries; keys bypass snapping.
export function snapTime(scale, position, available, width) {
  let seconds = scale.time(position);
  const near = (time) => time >= scale.now && time <= scale.end &&
    Math.abs(scale.position(time) - position) * Math.max(1, width - TIMELINE.thumbPixels) <= TIMELINE.snapPixels;
  // Reload readiness wins over a nearby command tick that may still be blocked.
  const ready = available.filter(near).sort((a, b) => Math.abs(a - seconds) - Math.abs(b - seconds))[0];
  const boundary = Math.round(seconds / scale.interval) * scale.interval;
  if (ready !== undefined) seconds = ready;
  else if (boundary > scale.now && Math.abs(seconds - boundary) <= TIMELINE.commandSnapSeconds && near(boundary)) seconds = boundary;
  return seconds;
}
