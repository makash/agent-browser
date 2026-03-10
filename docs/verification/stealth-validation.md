# Stealth Validation Workflow

This workflow is designed to answer a narrow question reliably:

1. Does stealth patching land before page JavaScript runs?
2. Does the resulting fingerprint surface match the intended 7-patch profile?
3. Can we capture evidence as both screenshots and machine-readable JSON?

The local probe page is the primary source of truth. Public detector pages are secondary diagnostics.

## Files

- Probe page: [test/e2e/fixtures/stealth-probe.html](/Users/mainstreet/code/agent-browser-stealth/agent-browser/test/e2e/fixtures/stealth-probe.html)
- Scorecard: [docs/verification/stealth-scorecard.json](/Users/mainstreet/code/agent-browser-stealth/agent-browser/docs/verification/stealth-scorecard.json)
- Spider chart: [docs/verification/stealth-parity-spider.svg](/Users/mainstreet/code/agent-browser-stealth/agent-browser/docs/verification/stealth-parity-spider.svg)
- Evidence: [docs/verification/evidence/2026-03-10-stealth](/Users/mainstreet/code/agent-browser-stealth/agent-browser/docs/verification/evidence/2026-03-10-stealth)

## Measured result

The local probe was executed on 2026-03-10 against all four profiles.

| Profile | Probe score |
| --- | --- |
| agent-browser normal | 4/7 |
| agent-browser stealth | 7/7 |
| Playwright normal | 1/7 |
| Playwright stealth | 6/7 |

Notes:

- `agent-browser stealth` passed the full probe.
- `Playwright stealth` passed everything except the hardware profile check because `connection.rtt` stayed at `50` instead of the probe's expected `100`.
- permissions passed in every run, so the spider chart focuses on the six axes that actually separated the profiles.

## Why this probe is reliable

The page captures its first fingerprint snapshot from an inline script inside `<head>`. If `stealth` or `init-script` injection is truly pre-navigation, the early snapshot already reflects the patched values.

The probe checks:

- `navigator.webdriver`
- `window.chrome`
- `navigator.plugins`
- `navigator.languages` and `navigator.language`
- WebGL vendor and renderer
- notification permissions query behavior
- hardware profile values (`hardwareConcurrency`, `deviceMemory`, `platform`, `connection.rtt`)

## Start the local probe server

Serve the fixture directory over HTTP so the page behaves like a normal origin:

```bash
cd /Users/mainstreet/code/agent-browser-stealth/agent-browser
python3 -m http.server 4173 --directory test/e2e/fixtures
```

Probe URL:

```text
http://127.0.0.1:4173/stealth-probe.html
```

## Evidence capture strategy

Capture two artifacts per run:

- a screenshot of the rendered probe page
- the JSON report from `window.__stealth_probe_report`

Name outputs by tool and mode:

- `agent-browser-normal.png`
- `agent-browser-normal.json`
- `agent-browser-stealth.png`
- `agent-browser-stealth.json`
- `playwright-normal.png`
- `playwright-normal.json`
- `playwright-stealth.png`
- `playwright-stealth.json`

Store them under a timestamped evidence directory such as `docs/verification/evidence/2026-03-10-stealth/`.

## Run 1: agent-browser normal

Use a clean session with no stealth preset:

```bash
cd /Users/mainstreet/code/agent-browser-stealth/agent-browser
BIN=./cli/target/release/agent-browser
export AGENT_BROWSER_NATIVE=1

$BIN --session ab-normal close || true
$BIN --session ab-normal open http://127.0.0.1:4173/stealth-probe.html
$BIN --session ab-normal screenshot evidence/2026-03-10-stealth/agent-browser-normal.png
$BIN --session ab-normal eval "JSON.stringify(window.__stealth_probe_report)" > evidence/2026-03-10-stealth/agent-browser-normal.json
```

## Run 2: agent-browser stealth

Enable the built-in preset before navigation:

```bash
cd /Users/mainstreet/code/agent-browser-stealth/agent-browser
BIN=./cli/target/release/agent-browser
export AGENT_BROWSER_NATIVE=1

$BIN --session ab-stealth close || true
$BIN --session ab-stealth stealth enable
$BIN --session ab-stealth open http://127.0.0.1:4173/stealth-probe.html
$BIN --session ab-stealth screenshot evidence/2026-03-10-stealth/agent-browser-stealth.png
$BIN --session ab-stealth eval "JSON.stringify(window.__stealth_probe_report)" > evidence/2026-03-10-stealth/agent-browser-stealth.json
```

Optional timing sanity check:

```bash
$BIN --session ab-init close || true
$BIN --session ab-init init-script add --js "window.__agent_init_probe={armed:true,ts:Date.now()}"
$BIN --session ab-init open http://127.0.0.1:4173/stealth-probe.html
$BIN --session ab-init eval "JSON.stringify(window.__agent_init_probe)"
```

## Run 3: Playwright normal

Run this from the `spa-hacker-agent` Python environment that already has Playwright available.

```python
import json
from pathlib import Path
from playwright.async_api import async_playwright
import asyncio

URL = "http://127.0.0.1:4173/stealth-probe.html"
OUT = Path("evidence/2026-03-10-stealth")
OUT.mkdir(parents=True, exist_ok=True)

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page()
        await page.goto(URL, wait_until="load")
        await page.screenshot(path=str(OUT / "playwright-normal.png"), full_page=True)
        report = await page.evaluate("window.__stealth_probe_report")
        (OUT / "playwright-normal.json").write_text(json.dumps(report, indent=2))
        await browser.close()

asyncio.run(main())
```

## Run 4: Playwright stealth

Use the same Playwright environment, but inject the stealth scripts before navigation. The current scanner entry points are in [stealth.py](/Users/mainstreet/code/spa-hacker/spa-hacker-agent/tools/stealth.py#L299) and [stealth.py](/Users/mainstreet/code/spa-hacker/spa-hacker-agent/tools/stealth.py#L313).

```python
import json
from pathlib import Path
from playwright.async_api import async_playwright
from tools.stealth import apply_stealth_to_page
import asyncio

URL = "http://127.0.0.1:4173/stealth-probe.html"
OUT = Path("evidence/2026-03-10-stealth")
OUT.mkdir(parents=True, exist_ok=True)

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page()
        await apply_stealth_to_page(page)
        await page.goto(URL, wait_until="load")
        await page.screenshot(path=str(OUT / "playwright-stealth.png"), full_page=True)
        report = await page.evaluate("window.__stealth_probe_report")
        (OUT / "playwright-stealth.json").write_text(json.dumps(report, indent=2))
        await browser.close()

asyncio.run(main())
```

## Public canary pages

After the local probe, run the same `normal` and `stealth` pairs against:

- `https://bot.sannysoft.com/`
- `https://www.browserscan.net/bot-detection`
- `https://abrahamjuliot.github.io/creepjs/`

For those runs, capture:

- page screenshot
- page title
- final URL
- challenge result if applicable

The public canaries are not ground truth for patch timing. They are useful for seeing whether the stealth profile improves the external detection posture.

## How to read the spider chart

The spider chart is intentionally narrow. It compares the local stealth probe surface only:

- webdriver
- chrome runtime
- plugins
- languages
- webgl
- hardware profile

Interpretation:

- `normal` profiles should score low because they do not actively patch the surface.
- `stealth` profiles should overlap closely if parity is working.
- If `agent-browser stealth` sits below `playwright stealth`, the missing area tells you which probe surface still needs work.

## Recommended evaluation order

1. local probe page
2. public canary pages
3. a small real-target canary set with screenshots and challenge-rate logging

That order keeps timing validation separate from internet noise.
