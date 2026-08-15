export interface Point { x: number; y: number }

export function normalizedVideoPoint(
  clientX: number, clientY: number,
  bounds: Pick<DOMRect, "left" | "top" | "width" | "height">,
  videoWidth: number, videoHeight: number,
): Point | null {
  if (videoWidth <= 0 || videoHeight <= 0 || bounds.width <= 0 || bounds.height <= 0) return null;
  const scale = Math.min(bounds.width / videoWidth, bounds.height / videoHeight);
  const width = videoWidth * scale;
  const height = videoHeight * scale;
  const left = bounds.left + (bounds.width - width) / 2;
  const top = bounds.top + (bounds.height - height) / 2;
  if (clientX < left || clientX > left + width || clientY < top || clientY > top + height) return null;
  return { x: (clientX - left) / width, y: (clientY - top) / height };
}

export function createMoveCoalescer(send: (point: Point) => void, schedule: (callback: () => void) => number = requestAnimationFrame) {
  let pending: Point | null = null;
  let scheduled = false;
  return (point: Point) => {
    pending = point;
    if (scheduled) return;
    scheduled = true;
    schedule(() => { scheduled = false; const latest = pending; pending = null; if (latest) send(latest); });
  };
}
