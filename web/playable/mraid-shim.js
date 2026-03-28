// web/playable/mraid-shim.js
//
// MRAID 2.0 compatibility layer for Arachne Playable Ads.
// Detects real MRAID SDK or provides mock for local testing.

(function () {
  'use strict';

  const listeners = {};

  function on(event, fn) {
    (listeners[event] = listeners[event] || []).push(fn);
  }

  function off(event, fn) {
    const list = listeners[event];
    if (list) listeners[event] = list.filter(f => f !== fn);
  }

  function emit(event, ...args) {
    (listeners[event] || []).forEach(fn => fn(...args));
  }

  // --- Real MRAID path ---
  if (typeof window.mraid !== 'undefined') {
    const mraid = window.mraid;

    window.ArachneMRAID = {
      isReal: true,

      ready(callback) {
        if (mraid.getState() === 'ready' || mraid.getState() === 'default') {
          callback();
        } else {
          mraid.addEventListener('ready', function onReady() {
            mraid.removeEventListener('ready', onReady);
            callback();
          });
        }
      },

      isViewable() {
        return mraid.isViewable();
      },

      onViewable(callback) {
        mraid.addEventListener('viewableChange', callback);
      },

      offViewable(callback) {
        mraid.removeEventListener('viewableChange', callback);
      },

      openStore(url) {
        mraid.open(url);
      },

      close() {
        mraid.close();
      },

      getState() {
        return mraid.getState();
      },

      onStateChange(callback) {
        mraid.addEventListener('stateChange', callback);
      },
    };
    return;
  }

  // --- Mock MRAID path (local testing) ---
  let mockViewable = true;
  let mockState = 'default';

  window.ArachneMRAID = {
    isReal: false,

    ready(callback) {
      if (document.readyState === 'complete' || document.readyState === 'interactive') {
        setTimeout(callback, 0);
      } else {
        document.addEventListener('DOMContentLoaded', () => setTimeout(callback, 0));
      }
    },

    isViewable() {
      return mockViewable;
    },

    onViewable(callback) {
      on('viewableChange', callback);
      document.addEventListener('visibilitychange', () => {
        mockViewable = !document.hidden;
        emit('viewableChange', mockViewable);
      });
    },

    offViewable(callback) {
      off('viewableChange', callback);
    },

    openStore(url) {
      console.log('[ArachneMRAID mock] openStore:', url);
      window.open(url, '_blank');
    },

    close() {
      console.log('[ArachneMRAID mock] close');
      mockState = 'hidden';
      emit('stateChange', mockState);
    },

    getState() {
      return mockState;
    },

    onStateChange(callback) {
      on('stateChange', callback);
    },
  };
})();
