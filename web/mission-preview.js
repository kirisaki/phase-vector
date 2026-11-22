// Observation-only estimate shared by the map marker and weapon feedback.
// The shot solver supplies arrival time, including flight after the selected firing time.
export function shotArrivalEstimate(state, solution) {
  if (!solution) return null;
  const { prediction, contact } = state.mission;
  const secondsFromNow = solution.arrival_seconds - state.time_seconds;
  const position = {
    x: prediction[0].x + contact.velocity.x * secondsFromNow,
    y: prediction[0].y + contact.velocity.y * secondsFromNow,
  };
  return { position, secondsFromNow,
    offsetMetres: Math.hypot(solution.target.x - position.x, solution.target.y - position.y) };
}
