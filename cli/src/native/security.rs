pub const DEFAULT_STEALTH_PRESET: &str = r#"
(() => {
  try {
    Object.defineProperty(navigator, 'webdriver', {
      get: () => undefined,
    });
    try { delete Object.getPrototypeOf(navigator).webdriver; } catch (_) {}

    if (!window.chrome) {
      window.chrome = {
        runtime: {
          onConnect: undefined,
          onMessage: undefined,
        },
        loadTimes: function() {
          return {
            commitLoadTime: Date.now() / 1000,
            connectionInfo: 'http/1.1',
            finishDocumentLoadTime: Date.now() / 1000,
            finishLoadTime: Date.now() / 1000,
            firstPaintAfterLoadTime: 0,
            firstPaintTime: Date.now() / 1000,
            navigationType: 'Other',
            npnNegotiatedProtocol: 'unknown',
            requestTime: Date.now() / 1000,
            startLoadTime: Date.now() / 1000,
            wasAlternateProtocolAvailable: false,
            wasFetchedViaSpdy: false,
            wasNpnNegotiated: false,
          };
        },
        csi: function() {
          return {
            startE: Date.now(),
            onloadT: Date.now(),
            pageT: Date.now(),
          };
        },
        app: {
          isInstalled: false,
          InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
          RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' },
        },
      };
    }

    Object.defineProperty(navigator, 'plugins', {
      get: () => {
        const makeMimeType = (type, suffixes, description, plugin) => ({
          type, suffixes, description, enabledPlugin: plugin,
        });
        const makePlugin = (name, description, filename, mimeTypes) => {
          const plugin = { name, description, filename, length: mimeTypes.length };
          mimeTypes.forEach((mt, i) => {
            plugin[i] = makeMimeType(mt.type, mt.suffixes, mt.description, plugin);
          });
          return plugin;
        };
        const plugins = [
          makePlugin(
            'Chrome PDF Plugin',
            'Portable Document Format',
            'internal-pdf-viewer',
            [{ type: 'application/x-google-chrome-pdf', suffixes: 'pdf', description: 'Portable Document Format' }]
          ),
          makePlugin(
            'Chrome PDF Viewer',
            '',
            'mhjfbmdgcfjbbpaeojofohoefgiehjai',
            [{ type: 'application/pdf', suffixes: 'pdf', description: '' }]
          ),
          makePlugin(
            'Native Client',
            '',
            'internal-nacl-plugin',
            [{ type: 'application/x-nacl', suffixes: '', description: 'Native Client Executable' }]
          ),
        ];
        plugins.length = 3;
        return plugins;
      },
    });

    Object.defineProperty(navigator, 'mimeTypes', {
      get: () => {
        const mimeTypes = [
          { type: 'application/pdf', suffixes: 'pdf', description: '' },
          { type: 'application/x-google-chrome-pdf', suffixes: 'pdf', description: 'Portable Document Format' },
          { type: 'application/x-nacl', suffixes: '', description: 'Native Client Executable' },
        ];
        mimeTypes.length = 3;
        return mimeTypes;
      },
    });

    Object.defineProperty(navigator, 'languages', {
      get: () => ['en-US', 'en'],
    });
    Object.defineProperty(navigator, 'language', {
      get: () => 'en-US',
    });

    const overrideWebGL = (proto) => {
      const orig = proto.getParameter;
      proto.getParameter = function(param) {
        if (param === 37445) return 'Intel Inc.';
        if (param === 37446) return 'Intel Iris OpenGL Engine';
        return orig.call(this, param);
      };
    };
    if (typeof WebGLRenderingContext !== 'undefined') {
      overrideWebGL(WebGLRenderingContext.prototype);
    }
    if (typeof WebGL2RenderingContext !== 'undefined') {
      overrideWebGL(WebGL2RenderingContext.prototype);
    }

    if (navigator.permissions) {
      const originalQuery = navigator.permissions.query.bind(navigator.permissions);
      navigator.permissions.query = (parameters) => {
        if (parameters && parameters.name === 'notifications') {
          return Promise.resolve({ state: Notification.permission });
        }
        return originalQuery(parameters);
      };
    }

    Object.defineProperty(navigator, 'hardwareConcurrency', {
      get: () => 8,
    });
    Object.defineProperty(navigator, 'deviceMemory', {
      get: () => 8,
    });
    Object.defineProperty(navigator, 'platform', {
      get: () => 'Win32',
    });

    if (navigator.connection) {
      Object.defineProperty(navigator.connection, 'rtt', {
        get: () => 100,
      });
    }

    // Canvas fingerprint noise — seeded PRNG for deterministic noise per session
    (() => {
      const seed = Math.floor(Math.random() * 2147483647);
      let s = seed;
      const prng = () => { s = (s * 16807 + 0) % 2147483647; return (s & 0xff) - 128; };

      const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
      const origToBlob = HTMLCanvasElement.prototype.toBlob;
      const origGetImageData = CanvasRenderingContext2D.prototype.getImageData;

      const noisifyImageData = (imageData) => {
        const d = imageData.data;
        let localSeed = seed;
        for (let i = 0; i < d.length && i < 1024; i += 4) {
          localSeed = (localSeed * 16807 + 0) % 2147483647;
          const noise = (localSeed & 3) - 1; // -1, 0, 1, or 2
          d[i] = Math.max(0, Math.min(255, d[i] + noise));
        }
        return imageData;
      };

      Object.defineProperty(CanvasRenderingContext2D.prototype, 'getImageData', {
        value: function() {
          const imageData = origGetImageData.apply(this, arguments);
          return noisifyImageData(imageData);
        },
        writable: true,
        configurable: true,
      });
      // Preserve toString
      CanvasRenderingContext2D.prototype.getImageData.toString = () => 'function getImageData() { [native code] }';

      Object.defineProperty(HTMLCanvasElement.prototype, 'toDataURL', {
        value: function() {
          const ctx = this.getContext('2d');
          if (ctx) {
            try {
              const imageData = origGetImageData.call(ctx, 0, 0, this.width, this.height);
              noisifyImageData(imageData);
              ctx.putImageData(imageData, 0, 0);
            } catch (_) {}
          }
          return origToDataURL.apply(this, arguments);
        },
        writable: true,
        configurable: true,
      });
      HTMLCanvasElement.prototype.toDataURL.toString = () => 'function toDataURL() { [native code] }';

      Object.defineProperty(HTMLCanvasElement.prototype, 'toBlob', {
        value: function() {
          const ctx = this.getContext('2d');
          if (ctx) {
            try {
              const imageData = origGetImageData.call(ctx, 0, 0, this.width, this.height);
              noisifyImageData(imageData);
              ctx.putImageData(imageData, 0, 0);
            } catch (_) {}
          }
          return origToBlob.apply(this, arguments);
        },
        writable: true,
        configurable: true,
      });
      HTMLCanvasElement.prototype.toBlob.toString = () => 'function toBlob() { [native code] }';
    })();

    // UA/Client Hints auto-discovery from stealth proxy
    (() => {
      let version = '133.0.0.0';
      try {
        const xhr = new XMLHttpRequest();
        xhr.open('GET', 'http://127.0.0.1:8080/__stealth/profile', false);
        xhr.timeout = 500;
        xhr.send();
        if (xhr.status === 200) {
          const profile = JSON.parse(xhr.responseText);
          if (profile.version) version = profile.version;
        }
      } catch (_) {}

      const majorVersion = version.split('.')[0];
      const brands = [
        { brand: 'Chromium', version: majorVersion },
        { brand: 'Google Chrome', version: majorVersion },
        { brand: 'Not-A.Brand', version: '8' },
      ];
      const ua = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/' + version + ' Safari/537.36';

      Object.defineProperty(navigator, 'userAgent', { get: () => ua });
      if (navigator.userAgentData) {
        Object.defineProperty(navigator.userAgentData, 'brands', { get: () => brands });
        Object.defineProperty(navigator.userAgentData, 'platform', { get: () => 'Windows' });
        Object.defineProperty(navigator.userAgentData, 'mobile', { get: () => false });
      }
    })();
  } catch (_) {}
})();
"#;

pub const CLOUDFLARE_DETECT_JS: &str = r#"
(() => {
  const title = document.title || '';
  const body = document.body ? document.body.innerText : '';

  if (title.includes('Just a moment')) return 'cf_challenge';
  if (title.includes('Attention Required')) return 'cf_block';
  if (title.includes('Access denied')) return 'cf_block';
  if (document.querySelector('#cf-challenge-running')) return 'cf_challenge';
  if (document.querySelector('.cf-browser-verification')) return 'cf_challenge';
  if (document.querySelector('#challenge-form')) return 'cf_challenge';
  if (body.includes('Checking if the site connection is secure')) return 'cf_challenge';
  if (body.includes('Enable JavaScript and cookies to continue')) return 'cf_challenge';

  return 'none';
})()
"#;

pub const CLOUDFLARE_WAIT_JS: &str = r#"
(() => {
  const title = document.title || '';
  return !title.includes('Just a moment')
    && !document.querySelector('#cf-challenge-running')
    && !document.querySelector('.cf-browser-verification');
})()
"#;
