// web/playable/arachne-playable.js
//
// Playable ad lifecycle wrapper for Arachne engine.
// Handles: auto-resize, CTA overlay, end card, pause/resume, analytics hooks.
// Requires: mraid-shim.js loaded first, arachne WASM module available.

(function () {
  'use strict';

  class ArachnePlayable {
    constructor(options) {
      this.opts = Object.assign({
        container: '#ad-container',
        canvasId: 'arachne-canvas',
        width: 320,
        height: 480,
        storeUrl: 'https://example.com',
        ctaText: 'Play Now!',
        endCardDelay: 15,
        maxInteractions: 0,
        onStart: null,
        onInteract: null,
        onCTA: null,
        onComplete: null,
      }, options);

      this._started = false;
      this._ended = false;
      this._interactionCount = 0;
      this._startTime = 0;
      this._endCardTimer = null;
      this._container = null;
      this._canvas = null;
      this._ctaOverlay = null;

      this._init();
    }

    _init() {
      this._container = typeof this.opts.container === 'string'
        ? document.querySelector(this.opts.container)
        : this.opts.container;

      if (!this._container) {
        console.error('[ArachnePlayable] Container not found:', this.opts.container);
        return;
      }

      const pos = getComputedStyle(this._container).position;
      if (pos === 'static') this._container.style.position = 'relative';

      this._canvas = this._container.querySelector('#' + this.opts.canvasId);
      if (!this._canvas) {
        this._canvas = document.createElement('canvas');
        this._canvas.id = this.opts.canvasId;
        this._container.appendChild(this._canvas);
      }
      this._canvas.width = this.opts.width;
      this._canvas.height = this.opts.height;
      this._canvas.style.width = '100%';
      this._canvas.style.height = '100%';
      this._canvas.style.display = 'block';
      this._canvas.style.touchAction = 'none';

      this._buildCTAOverlay();

      const interactionHandler = () => {
        this._interactionCount++;
        if (this.opts.onInteract) this.opts.onInteract(this._interactionCount);
        if (this.opts.maxInteractions > 0 &&
            this._interactionCount >= this.opts.maxInteractions &&
            !this._ended) {
          this.showEndCard();
        }
      };
      this._canvas.addEventListener('pointerdown', interactionHandler);
      this._canvas.addEventListener('touchstart', interactionHandler, { passive: true });

      this._resizeObserver = new ResizeObserver(() => this._fitCanvas());
      this._resizeObserver.observe(this._container);
      this._fitCanvas();
    }

    _fitCanvas() {
      // Canvas CSS already set to 100% of container.
      // The container dictates the visual size; canvas resolution stays fixed.
    }

    _buildCTAOverlay() {
      this._ctaOverlay = document.createElement('div');
      this._ctaOverlay.style.cssText = `
        position: absolute; inset: 0;
        display: none; flex-direction: column;
        align-items: center; justify-content: center;
        background: rgba(0,0,0,0.75);
        z-index: 100; cursor: pointer;
      `;

      const btn = document.createElement('button');
      btn.textContent = this.opts.ctaText;
      btn.style.cssText = `
        padding: 16px 48px; font-size: 22px; font-weight: bold;
        background: #4CAF50; color: white; border: none;
        border-radius: 12px; cursor: pointer;
        box-shadow: 0 4px 15px rgba(0,0,0,0.3);
        transition: transform 0.1s;
      `;
      btn.addEventListener('pointerdown', () => { btn.style.transform = 'scale(0.95)'; });
      btn.addEventListener('pointerup', () => { btn.style.transform = 'scale(1)'; });
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        this._onCTAClick();
      });

      this._ctaOverlay.appendChild(btn);
      this._ctaOverlay.addEventListener('click', () => this._onCTAClick());
      this._container.appendChild(this._ctaOverlay);
    }

    _onCTAClick() {
      if (this.opts.onCTA) this.opts.onCTA();
      ArachneMRAID.openStore(this.opts.storeUrl);
    }

    start(initFn) {
      if (this._started) return;

      ArachneMRAID.ready(async () => {
        this._started = true;
        this._startTime = Date.now();
        if (this.opts.onStart) this.opts.onStart();

        ArachneMRAID.onViewable(() => {
          // With run(), the rAF loop auto-pauses when tab is hidden.
        });

        if (this.opts.endCardDelay > 0) {
          this._endCardTimer = setTimeout(
            () => { if (!this._ended) this.showEndCard(); },
            this.opts.endCardDelay * 1000
          );
        }

        if (initFn) {
          await initFn(this._canvas.id);
        }
      });
    }

    showEndCard() {
      if (this._ended) return;
      this._ended = true;
      if (this._endCardTimer) clearTimeout(this._endCardTimer);
      this._ctaOverlay.style.display = 'flex';
      if (this.opts.onComplete) this.opts.onComplete();
    }

    hideEndCard() {
      this._ctaOverlay.style.display = 'none';
    }

    get interactionCount() { return this._interactionCount; }
    get elapsed() { return this._started ? (Date.now() - this._startTime) / 1000 : 0; }
    get hasEnded() { return this._ended; }

    destroy() {
      if (this._resizeObserver) this._resizeObserver.disconnect();
      if (this._endCardTimer) clearTimeout(this._endCardTimer);
      if (this._ctaOverlay && this._ctaOverlay.parentNode) {
        this._ctaOverlay.parentNode.removeChild(this._ctaOverlay);
      }
    }
  }

  window.ArachnePlayable = ArachnePlayable;
})();
