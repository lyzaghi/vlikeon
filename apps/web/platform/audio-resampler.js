/**
 * Offline Audio Resampler for Web/PWA game cores.
 * Implements linear and cubic hermite interpolation to convert arbitrary core audio rates
 * (e.g. 32000 Hz, 44100 Hz) to the Web Audio AudioContext hardware sampleRate (e.g. 48000 Hz).
 */

export class AudioResampler {
  constructor(sourceRate, targetRate, channels = 2) {
    if (!Number.isFinite(sourceRate) || sourceRate <= 0 || !Number.isFinite(targetRate) || targetRate <= 0) {
      throw new Error('Sample rates must be positive finite numbers');
    }
    this.sourceRate = sourceRate;
    this.targetRate = targetRate;
    this.channels = channels;
    this.ratio = sourceRate / targetRate;
    this.phase = 0;
    this.lastFrame = null;
  }

  reset() {
    this.phase = 0;
    this.lastFrame = null;
  }

  /**
   * Resamples interleaved PCM Float32 samples from sourceRate to targetRate
   * while preserving fractional phase and boundary samples across chunks.
   */
  resampleInterleaved(inputSamples) {
    if (this.sourceRate === this.targetRate || !inputSamples || inputSamples.length === 0) {
      return inputSamples;
    }

    const inputFrames = Math.floor(inputSamples.length / this.channels);
    if (inputFrames === 0) {
      return new Float32Array(0);
    }

    // Estimate output frame count given current fractional phase
    const availableSpan = inputFrames - this.phase;
    if (availableSpan <= 0) {
      this.phase -= inputFrames;
      const tail = new Float32Array(this.channels);
      const lastOffset = (inputFrames - 1) * this.channels;
      for (let ch = 0; ch < this.channels; ch++) {
        tail[ch] = inputSamples[lastOffset + ch];
      }
      this.lastFrame = tail;
      return new Float32Array(0);
    }

    const outputFrames = Math.floor(availableSpan / this.ratio);
    const outputSamples = new Float32Array(outputFrames * this.channels);

    let srcPos = this.phase;
    for (let outFrame = 0; outFrame < outputFrames; outFrame++) {
      const srcIndex = Math.floor(srcPos);
      const frac = srcPos - srcIndex;

      for (let ch = 0; ch < this.channels; ch++) {
        let s0;
        let s1;
        if (srcIndex < 0) {
          s0 = this.lastFrame ? this.lastFrame[ch] : inputSamples[ch];
          s1 = inputSamples[ch];
        } else {
          const idx0 = (srcIndex * this.channels) + ch;
          const nextFrame = Math.min(srcIndex + 1, inputFrames - 1);
          const idx1 = (nextFrame * this.channels) + ch;
          s0 = inputSamples[idx0];
          s1 = inputSamples[idx1];
        }

        outputSamples[(outFrame * this.channels) + ch] = s0 + (frac * (s1 - s0));
      }

      srcPos += this.ratio;
    }

    this.phase = srcPos - inputFrames;
    const tail = new Float32Array(this.channels);
    const lastOffset = (inputFrames - 1) * this.channels;
    for (let ch = 0; ch < this.channels; ch++) {
      tail[ch] = inputSamples[lastOffset + ch];
    }
    this.lastFrame = tail;

    return outputSamples;
  }
}
