// Presentation and input tuning only. Simulation settings come from state_json().
export const AIM = Object.freeze({
  snapPixels: 12,
  fineGain: 0.35,
  stepMetres: 100000,
  fineStepMetres: 10000,
});

export const AIM_DETAIL = Object.freeze({
  magnification: 8,
  lingerMs: 1500,
  markerInsetPixels: 10,
  scaleAreaPixels: 24,
  edgePaddingPixels: 8,
  handleGapPixels: 32,
  pointerClearancePixels: 24,
});

export const TIMELINE = Object.freeze({
  nearShare: 0.4,
  snapPixels: 5,
  commandSnapSeconds: 6,
  thumbPixels: 14,
});

export const NAVIGATION = Object.freeze({
  dragThresholdPixels: 4,
  minPinchSpanPixels: 8,
});

export const BURN = Object.freeze({
  minRadiusPixels: 24,
  maxRadiusPixels: 90,
  edgePaddingPixels: 10,
  keyFraction: 0.02,
  fineKeyFraction: 0.002,
});

export const COMMAND_PULSE = Object.freeze({
  durationMs: 900,
  opacity: 0.85,
  startRadiusPixels: 12,
  growthPixels: 42,
});

// History length is independent of the mission's reservation capacity.
export const RECENT_LAUNCHES = 4;
