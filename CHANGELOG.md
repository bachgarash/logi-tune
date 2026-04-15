# Changelog

## [0.1.1](https://github.com/bachgarash/logi-tune/compare/v0.1.0...v0.1.1) (2026-04-15)


### Bug Fixes

* retry evdev monitor open to survive udev permission race ([c8a1772](https://github.com/bachgarash/logi-tune/commit/c8a1772075a0e3900f37bc9467d07cf0de95f742))

## 0.1.0 (2026-04-01)


### Bug Fixes

* detect focus loss to non-AT-SPI2 apps like Chrome ([cc1b95e](https://github.com/bachgarash/logi-tune/commit/cc1b95ec80a82226826a3e1c42d43483bef1ae4f))
* exit process when monitor thread loses device so systemd restarts ([5607933](https://github.com/bachgarash/logi-tune/commit/5607933413be6d8da0ca9514809b771251701534))
* exit with error in daemon mode when device not found ([4dff985](https://github.com/bachgarash/logi-tune/commit/4dff985e9a9135fce88e03810a3f003a6b6aaa52))
* poll focus tracker continuously instead of stopping after 60s ([7dd16c3](https://github.com/bachgarash/logi-tune/commit/7dd16c31fe267c5dd274c34897223a983c7551e3))
* resolve all clippy and rustfmt lint warnings ([7eb044a](https://github.com/bachgarash/logi-tune/commit/7eb044a3814847d09c9b6b01c79f0c3ebe7ff446))
