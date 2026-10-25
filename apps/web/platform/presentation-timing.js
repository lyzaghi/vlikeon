// Presentation Timing Instrumentation for Web / PWA Runtime
// Measures real render-to-present, swapchain latency, jitter, and frame drops.
// Bounded queue: max 2 buffered frames, sliding window of at most 120 samples.

export const DropReason = Object.freeze({
  QUEUE_OVERFLOW: 'queue_overflow',
  LATE_FRAME: 'late_frame',
  FOCUS_LOSS: 'focus_loss',
  THERMAL_THROTTLE: 'thermal_throttle',
  DISPLAY_DISCONNECTED: 'display_disconnected',
});

export class PresentationTimingCollector {
  #maxSamples;
  #records;
  #framesEmulated;
  #framesPresented;
  #framesDropped;
  #queueDepth;
  #maxQueueDepth;
  #dacStarvations;
  #backend;

  constructor(backend = 'webgl_canvas', maxSamples = 120) {
    this.#backend = backend;
    this.#maxSamples = Math.min(Math.max(10, maxSamples), 240);
    this.#records = [];
    this.#framesEmulated = 0;
    this.#framesPresented = 0;
    this.#framesDropped = 0;
    this.#queueDepth = 0;
    this.#maxQueueDepth = 2;
    this.#dacStarvations = 0;
  }

  get queueDepth() {
    return this.#queueDepth;
  }

  recordFrameDispatch(frameIndex, renderTimestampNs) {
    this.#framesEmulated++;
    const enqueueNs = Math.round(performance.now() * 1_000_000);
    const renderNs = renderTimestampNs || enqueueNs;

    if (this.#queueDepth >= this.#maxQueueDepth) {
      this.recordFrameDropped(frameIndex, DropReason.QUEUE_OVERFLOW);
      return false;
    }

    this.#queueDepth++;
    return true;
  }

  recordFramePresented(frameIndex, renderTimestampNs, presentTimestampNs, flipTimestampNs = null) {
    if (this.#queueDepth > 0) {
      this.#queueDepth--;
    }
    this.#framesPresented++;

    const nowNs = Math.round(performance.now() * 1_000_000);
    const presentNs = presentTimestampNs || nowNs;
    const renderNs = renderTimestampNs || presentNs;

    const record = {
      frame_index: frameIndex,
      render_timestamp_ns: renderNs,
      enqueue_timestamp_ns: renderNs,
      present_timestamp_ns: presentNs,
      flip_timestamp_ns: flipTimestampNs !== undefined ? flipTimestampNs : null,
      dropped: false,
      drop_reason: null,
    };

    this.#records.push(record);
    if (this.#records.length > this.#maxSamples) {
      this.#records.shift();
    }
  }

  recordFrameDropped(frameIndex, reason = DropReason.LATE_FRAME) {
    if (this.#queueDepth > 0 && reason !== DropReason.QUEUE_OVERFLOW) {
      this.#queueDepth--;
    }
    this.#framesDropped++;

    const nowNs = Math.round(performance.now() * 1_000_000);
    const record = {
      frame_index: frameIndex,
      render_timestamp_ns: nowNs,
      enqueue_timestamp_ns: nowNs,
      present_timestamp_ns: nowNs,
      flip_timestamp_ns: null,
      dropped: true,
      drop_reason: reason,
    };

    this.#records.push(record);
    if (this.#records.length > this.#maxSamples) {
      this.#records.shift();
    }
  }

  recordAudioStarvation(count = 1) {
    this.#dacStarvations += Math.max(0, count);
  }

  getMetricsReport(sessionId) {
    const validRecords = this.#records.filter((r) => !r.dropped && r.present_timestamp_ns >= r.render_timestamp_ns);
    const renderToPresentLatencies = validRecords.map(
      (r) => (r.present_timestamp_ns - r.render_timestamp_ns) / 1_000_000
    );

    renderToPresentLatencies.sort((a, b) => a - b);

    const quantile = (arr, q) => {
      if (arr.length === 0) return 0.0;
      const index = Math.floor((arr.length - 1) * q);
      return arr[index];
    };

    // Frame jitter: difference between consecutive present intervals
    const jitters = [];
    for (let i = 1; i < validRecords.length; i++) {
      const prevDelta = (validRecords[i - 1].present_timestamp_ns - validRecords[i - 1].render_timestamp_ns) / 1_000_000;
      const currDelta = (validRecords[i].present_timestamp_ns - validRecords[i].render_timestamp_ns) / 1_000_000;
      jitters.push(Math.abs(currDelta - prevDelta));
    }
    jitters.sort((a, b) => a - b);

    const flipRecords = validRecords.filter(
      (r) => typeof r.flip_timestamp_ns === 'number' && r.flip_timestamp_ns >= r.present_timestamp_ns
    );
    const presentToFlipLatencies = flipRecords
      .map((r) => (r.flip_timestamp_ns - r.present_timestamp_ns) / 1_000_000)
      .sort((a, b) => a - b);

    return {
      session_id: sessionId,
      sample_at_ms: Date.now(),
      presentation_backend: this.#backend,
      frames_emulated: this.#framesEmulated,
      frames_presented: this.#framesPresented,
      frames_dropped: this.#framesDropped,
      queue_depth_current: this.#queueDepth,
      queue_depth_max: this.#maxQueueDepth,
      render_to_present_p50_ms: quantile(renderToPresentLatencies, 0.5),
      render_to_present_p95_ms: quantile(renderToPresentLatencies, 0.95),
      render_to_present_p99_ms: quantile(renderToPresentLatencies, 0.99),
      present_to_flip_p50_ms: presentToFlipLatencies.length > 0 ? quantile(presentToFlipLatencies, 0.5) : null,
      present_to_flip_p95_ms: presentToFlipLatencies.length > 0 ? quantile(presentToFlipLatencies, 0.95) : null,
      jitter_p95_ms: quantile(jitters, 0.95),
      audio_dac_starvations: this.#dacStarvations,
    };
  }
}
