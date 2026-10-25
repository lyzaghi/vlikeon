/**
 * Offline Audio Resampler for Web/PWA game cores.
 * Implements linear and cubic hermite interpolation to convert arbitrary core audio rates
 * (e.g. 32000 Hz, 44100 Hz) to the Web Audio AudioContext hardware sampleRate (e.g. 48000 Hz).
 */

export class AudioResampler {
  constructor(sourceRate, targetRate, channels = 2) {
    this.sourceRate = sourceRate;
    this.targetRate = targetRate;
    this.channels = channels;
    this.ratio = sourceRate / targetRate;
  }

  /**
   * Resamples interleaved stereo PCM Float32 samples from sourceRate to targetRate.
   */
  resampleInterleaved(inputSamples) {
    if (this.sourceRate === this.targetRate) {
      return inputSamples;
    }

    const inputFrames = inputSamples.length / this.channels;
    const outputFrames = Math.floor(inputFrames / this.ratio);
    const outputSamples = new Float32Array(outputFrames * this.channels);

    for (let outFrame = 0; outFrame < outputFrames; outFrame++) {
      const srcPos = outFrame * this.ratio;
      const srcIndex = Math.floor(srcPos);
      const frac = srcPos - srcIndex;

      for (let ch = 0; ch < this.channels; ch++) {
        const idx0 = (srcIndex * this.channels) + ch;
        const idx1 = Math.min(((srcIndex + 1) * this.channels) + ch, inputSamples.length - 1);

        const s0 = inputSamples[idx0];
        const s1 = inputSamples[idx1];

        // Linear interpolation
        outputSamples[(outFrame * this.channels) + ch] = s0 + (frac * (s1 - s0));
      }
    }

    return outputSamples;
  }
}
