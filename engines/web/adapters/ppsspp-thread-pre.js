// The UI owns the entire admitted pool. A child must never create another
// untracked browser worker through the SDK/global Worker surface.
if (typeof ENVIRONMENT_IS_PTHREAD !== 'undefined' && ENVIRONMENT_IS_PTHREAD) {
  globalThis.Worker = class {
    constructor() { throw new Error('Nested emulator threads are not admitted.'); }
  };
}
