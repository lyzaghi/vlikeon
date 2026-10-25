/**
 * Web/PWA Installation, Update, and Cache Recovery Manager.
 * - Handles beforeinstallprompt and appinstalled events.
 * - Coordinates background Service Worker updates without disrupting active game sessions.
 * - Provides automated cache corruption recovery and clean re-synchronization.
 */

export class PwaLifecycleManager {
  constructor() {
    this.deferredPrompt = null;
    this.isInstallable = false;
    this.isInstalled = false;
    this.updateWaiting = false;
    this.waitingWorker = null;
    this.gameSessionActive = false;

    this.listeners = new Set();
    this.init();
  }

  init() {
    window.addEventListener('beforeinstallprompt', (e) => {
      e.preventDefault();
      this.deferredPrompt = e;
      this.isInstallable = true;
      this.notify();
    });

    window.addEventListener('appinstalled', () => {
      this.deferredPrompt = null;
      this.isInstallable = false;
      this.isInstalled = true;
      this.notify();
    });

    if ('serviceWorker' in navigator) {
      navigator.serviceWorker.ready.then((registration) => {
        registration.addEventListener('updatefound', () => {
          const installing = registration.installing;
          if (!installing) return;

          installing.addEventListener('statechange', () => {
            if (installing.state === 'installed' && navigator.serviceWorker.controller) {
              this.updateWaiting = true;
              this.waitingWorker = installing;
              this.notify();
            }
          });
        });
      });
    }
  }

  setGameSessionActive(active) {
    this.gameSessionActive = active;
  }

  async promptInstall() {
    if (!this.deferredPrompt) return false;
    this.deferredPrompt.prompt();
    const { outcome } = await this.deferredPrompt.userChoice;
    this.deferredPrompt = null;
    this.isInstallable = false;
    this.notify();
    return outcome === 'accepted';
  }

  applyPendingUpdate() {
    if (!this.updateWaiting || !this.waitingWorker) return;
    if (this.gameSessionActive) {
      console.warn('Game session is currently active; delay update reload until save is committed.');
      return;
    }
    // Normal activation policy: reload page to claim fresh service worker
    window.location.reload();
  }

  /**
   * Purges all application caches and unregisters service workers on detected corruption.
   * Never touches user games, saves, or IndexedDB storage.
   */
  async recoverCache() {
    if ('caches' in window) {
      const keys = await caches.keys();
      for (const key of keys) {
        if (key.startsWith('omniplay-web:')) {
          await caches.delete(key);
        }
      }
    }
    if ('serviceWorker' in navigator) {
      const registrations = await navigator.serviceWorker.getRegistrations();
      for (const reg of registrations) {
        await reg.unregister();
      }
      await navigator.serviceWorker.register('./service-worker.js', { scope: './' });
    }
    window.location.reload();
  }

  subscribe(callback) {
    this.listeners.add(callback);
    return () => this.listeners.delete(callback);
  }

  notify() {
    const state = {
      isInstallable: this.isInstallable,
      isInstalled: this.isInstalled,
      updateWaiting: this.updateWaiting,
      gameSessionActive: this.gameSessionActive
    };
    for (const cb of this.listeners) {
      try { cb(state); } catch (e) { console.error(e); }
    }
  }
}

export const pwaManager = new PwaLifecycleManager();
