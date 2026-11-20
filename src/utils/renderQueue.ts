/**
 * Serial client-side thumbnail render queue.
 *
 * `renderThumbnailInBrowser` runs Three.js parsing + WebGL rendering on the
 * main thread. If a user scrolls past a wall of cards whose thumbnails are
 * still `pending`/`failed`, every card's useEffect fires at once and 20+
 * concurrent renders stall the UI for seconds.
 *
 * This queue serializes renders with a tiny inter-render gap so the browser
 * stays responsive. It is a singleton: every caller enqueues into the same
 * queue, and the queue drains itself on the next tick.
 */

type RenderJob = {
  modelId: string;
  filePath: string;
  onComplete?: (thumbnailPath: string) => void;
  onError?: (err: unknown) => void;
};

const INTER_RENDER_MS = 80; // breathing room between renders
const MAX_QUEUE_LENGTH = 50; // cap to bound memory if user scrolls insanely fast

let queue: RenderJob[] = [];
let draining = false;

function pump() {
  // Stop if a render is already in flight or the queue is empty.
  if (draining || queue.length === 0) return;

  // Trim the head of the queue so we never hold onto more than the cap.
  while (queue.length > MAX_QUEUE_LENGTH) {
    const dropped = queue.shift();
    dropped?.onError?.('Render queue overflow — skipped');
  }

  const job = queue[0];
  draining = true;

  // Lazy import to avoid pulling Three.js into the module graph until a
  // render is actually needed.
  import('./clientThumbnailRenderer')
    .then(({ renderThumbnailInBrowser }) =>
      renderThumbnailInBrowser(job.modelId, job.filePath, job.onComplete)
    )
    .catch((err) => job.onError?.(err))
    .finally(() => {
      draining = false;
      // Remove the job we just ran (or failed) before pumping again.
      if (queue[0] === job) queue.shift();
      // Give the event loop a beat before starting the next render so the
      // UI can paint scroll position, hover states, etc.
      setTimeout(pump, INTER_RENDER_MS);
    });
}

export function enqueueRender(job: RenderJob): boolean {
  // Dedupe: a model already queued or currently rendering is not re-added.
  // The in-flight job lives at queue[0], so checking the array covers both.
  if (queue.some((j) => j.modelId === job.modelId)) {
    return false;
  }
  queue.push(job);
  pump();
  return true;
}

export function getQueueDepth(): number {
  return queue.length + (draining ? 1 : 0);
}

export function cancelRender(modelId: string): void {
  queue = queue.filter((j) => j.modelId !== modelId);
}