# Changelog

## [0.3.0](https://github.com/bachgarash/logi-tune/compare/v0.2.0...v0.3.0) (2026-11-03)


### Features

* add --watch-buttons and make evdev button codes configurable ([38d612f](https://github.com/bachgarash/logi-tune/commit/38d612f6ae70ee48e713e3cb00398fddd36fb686))
* add --watch-buttons and make evdev button codes configurable ([b2a7e11](https://github.com/bachgarash/logi-tune/commit/b2a7e1147f02aae2b7dfff6002b319caf9fb1af5))
* add haptic motor support (0x19b0) for the MX Master 4 ([fbfc6e9](https://github.com/bachgarash/logi-tune/commit/fbfc6e96f07e351569efad6c899f1c859c38cc4c))
* add haptic motor support (0x19b0) for the MX Master 4 ([8b8ab2f](https://github.com/bachgarash/logi-tune/commit/8b8ab2fddd17cb9ef5abea463ac56ae0263bb8ea))
* make HID++ work over Bluetooth, add --scan, fix MX Master 4 control IDs ([e010670](https://github.com/bachgarash/logi-tune/commit/e010670aae0d0941273a934b31d17d7fdce98f01))
* make HID++ work over Bluetooth, add --scan, fix MX Master 4 control IDs ([ee43bb7](https://github.com/bachgarash/logi-tune/commit/ee43bb7436ff286a0ed8e6afd3ddc7ae77625751))


### Bug Fixes

* bind the MX Master 4's fifth button (evdev 278) ([c34a914](https://github.com/bachgarash/logi-tune/commit/c34a914ad22fb72ae21bb18664dd6e2ac6ba4ce3))
* correct the HID++ wire format of every device setting ([906a20a](https://github.com/bachgarash/logi-tune/commit/906a20a6c6368f79754bc428797d967634dd84d6))
* correct the HID++ wire format of every device setting ([8ae7610](https://github.com/bachgarash/logi-tune/commit/8ae7610a6980e591f5eb6d916df884d7ca8cf197))
* stop leaking the AT-SPI2 focus tracker process ([915781c](https://github.com/bachgarash/logi-tune/commit/915781c29b02c94417e8056b74f2e1455c84b3a8))
* stop leaking the AT-SPI2 focus tracker process ([369af12](https://github.com/bachgarash/logi-tune/commit/369af12dbe20401ea911011809409b8b60fa32a9))

## [0.2.0](https://github.com/bachgarash/logi-tune/compare/v0.1.3...v0.2.0) (2026-11-03)


### Features

* add MX Master 4 (Bluetooth) support ([903d7fd](https://github.com/bachgarash/logi-tune/commit/903d7fdca02e80f036519f3fed388c1a3fcfafc8))
* add MX Master 4 (Bluetooth) support ([19e516c](https://github.com/bachgarash/logi-tune/commit/19e516c799693766b76f6b9a6d93436028a02f95))

## [0.1.3](https://github.com/bachgarash/logi-tune/compare/v0.1.2...v0.1.3) (2026-11-03)


### Bug Fixes

* evaluate hidpp! command once and collapse nested ifs into match … ([ef5fb79](https://github.com/bachgarash/logi-tune/commit/ef5fb79972eda85d2bd4bada770e5253e2ae694b))
* evaluate hidpp! command once and collapse nested ifs into match guards ([7021fd5](https://github.com/bachgarash/logi-tune/commit/7021fd50bb152e3bf928bb17b658d3ac1690011b))

## [0.1.2](https://github.com/bachgarash/logi-tune/compare/v0.1.1...v0.1.2) (2026-04-29)


### Bug Fixes

* suppress orphaned MSC_SCAN and button releases for swallowed presses ([529825e](https://github.com/bachgarash/logi-tune/commit/529825ea146529ec92a374948325ea863359b44b))

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
