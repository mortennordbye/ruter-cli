# Changelog

## [0.6.0](https://github.com/mortennordbye/ruter-cli/compare/v0.5.0...v0.6.0) (2026-08-04)


### Features

* add curl installer and built-in `ruter upgrade` ([#4](https://github.com/mortennordbye/ruter-cli/issues/4)) ([484a984](https://github.com/mortennordbye/ruter-cli/commit/484a984dacc9a0c5930b2cec35592a62006f800e))
* **ci:** build and attach release binaries ([958bd60](https://github.com/mortennordbye/ruter-cli/commit/958bd60064b09867bed7c8384c2b33217de3bdb4))
* **cli:** accept a destination of several words without quotes ([#15](https://github.com/mortennordbye/ruter-cli/issues/15)) ([11b97c1](https://github.com/mortennordbye/ruter-cli/commit/11b97c1d7fe8b8c398c0cf74c14d19e96fd8ee4c))
* **install:** put ~/.local/bin on PATH, and lead the README with the install command ([#16](https://github.com/mortennordbye/ruter-cli/issues/16)) ([d2e33c0](https://github.com/mortennordbye/ruter-cli/commit/d2e33c00aae939ef9149885209d4232143c58c87))
* **render:** separate journeys with rules and align the itinerary columns ([#6](https://github.com/mortennordbye/ruter-cli/issues/6)) ([fe912c3](https://github.com/mortennordbye/ruter-cli/commit/fe912c3b5547f7176ec9b7f34bd66e4c8be06b01))
* **route:** save journeys that travel via specific stops ([#11](https://github.com/mortennordbye/ruter-cli/issues/11)) ([7e3e2ab](https://github.com/mortennordbye/ruter-cli/commit/7e3e2aba2959c6149ce5c8690d447bf4dd744b86))


### Bug Fixes

* **ci:** call the binary build from release-please instead of on release published ([b6f012e](https://github.com/mortennordbye/ruter-cli/commit/b6f012e97e6c0124848fb579bb3cf4afad766765))
* **ci:** publish the release only once its assets are uploaded ([#5](https://github.com/mortennordbye/ruter-cli/issues/5)) ([1649b72](https://github.com/mortennordbye/ruter-cli/commit/1649b72d4aad58c6d86ea4be2fb310634b211902))
* **ci:** take packaging helpers from the workflow ref, not the tag ([d8c9217](https://github.com/mortennordbye/ruter-cli/commit/d8c9217d1fa14ae8c343c1575e18154e59161296))
* **ci:** upload release assets with gh instead of action-gh-release ([130f135](https://github.com/mortennordbye/ruter-cli/commit/130f13501944efd06ed0c20f3a845eb87b106d0c))
* **location:** drive Core Location with a delegate so GPS stops falling back to IP ([#12](https://github.com/mortennordbye/ruter-cli/issues/12)) ([3a50440](https://github.com/mortennordbye/ruter-cli/commit/3a50440679c4f8d7922c1b4eb873ff5eeb3fc7ee))
* **location:** settle authorization before asking for a position ([#14](https://github.com/mortennordbye/ruter-cli/issues/14)) ([ce62435](https://github.com/mortennordbye/ruter-cli/commit/ce62435ca7890d69bb18ccfae94aa710e2f09cc0))
* **upgrade:** close the installer's stdin so the upgrade can finish ([#9](https://github.com/mortennordbye/ruter-cli/issues/9)) ([c2fb65c](https://github.com/mortennordbye/ruter-cli/commit/c2fb65cb85d1cb21883c8bdef5001344128da799))

## [0.5.0](https://github.com/mortennordbye/ruter-cli/compare/v0.4.0...v0.5.0) (2026-08-04)


### Features

* **cli:** accept a destination of several words without quotes ([#15](https://github.com/mortennordbye/ruter-cli/issues/15)) ([11b97c1](https://github.com/mortennordbye/ruter-cli/commit/11b97c1d7fe8b8c398c0cf74c14d19e96fd8ee4c))
* **install:** put ~/.local/bin on PATH, and lead the README with the install command ([#16](https://github.com/mortennordbye/ruter-cli/issues/16)) ([d2e33c0](https://github.com/mortennordbye/ruter-cli/commit/d2e33c00aae939ef9149885209d4232143c58c87))


### Bug Fixes

* **location:** settle authorization before asking for a position ([#14](https://github.com/mortennordbye/ruter-cli/issues/14)) ([ce62435](https://github.com/mortennordbye/ruter-cli/commit/ce62435ca7890d69bb18ccfae94aa710e2f09cc0))

## [0.4.0](https://github.com/mortennordbye/ruter-cli/compare/v0.3.0...v0.4.0) (2026-08-04)


### Features

* **route:** save journeys that travel via specific stops ([#11](https://github.com/mortennordbye/ruter-cli/issues/11)) ([7e3e2ab](https://github.com/mortennordbye/ruter-cli/commit/7e3e2aba2959c6149ce5c8690d447bf4dd744b86))


### Bug Fixes

* **location:** drive Core Location with a delegate so GPS stops falling back to IP ([#12](https://github.com/mortennordbye/ruter-cli/issues/12)) ([3a50440](https://github.com/mortennordbye/ruter-cli/commit/3a50440679c4f8d7922c1b4eb873ff5eeb3fc7ee))
* **upgrade:** close the installer's stdin so the upgrade can finish ([#9](https://github.com/mortennordbye/ruter-cli/issues/9)) ([c2fb65c](https://github.com/mortennordbye/ruter-cli/commit/c2fb65cb85d1cb21883c8bdef5001344128da799))

## [0.3.0](https://github.com/mortennordbye/ruter-cli/compare/v0.2.1...v0.3.0) (2026-07-31)


### Features

* **render:** separate journeys with rules and align the itinerary columns ([#6](https://github.com/mortennordbye/ruter-cli/issues/6)) ([fe912c3](https://github.com/mortennordbye/ruter-cli/commit/fe912c3b5547f7176ec9b7f34bd66e4c8be06b01))


### Bug Fixes

* **ci:** publish the release only once its assets are uploaded ([#5](https://github.com/mortennordbye/ruter-cli/issues/5)) ([1649b72](https://github.com/mortennordbye/ruter-cli/commit/1649b72d4aad58c6d86ea4be2fb310634b211902))

## [0.2.1](https://github.com/mortennordbye/ruter-cli/compare/v0.2.0...v0.2.1) (2026-07-31)


### Bug Fixes

* **ci:** call the binary build from release-please instead of on release published ([b6f012e](https://github.com/mortennordbye/ruter-cli/commit/b6f012e97e6c0124848fb579bb3cf4afad766765))
* **ci:** take packaging helpers from the workflow ref, not the tag ([d8c9217](https://github.com/mortennordbye/ruter-cli/commit/d8c9217d1fa14ae8c343c1575e18154e59161296))
* **ci:** upload release assets with gh instead of action-gh-release ([130f135](https://github.com/mortennordbye/ruter-cli/commit/130f13501944efd06ed0c20f3a845eb87b106d0c))

## [0.2.0](https://github.com/mortennordbye/ruter-cli/compare/v0.1.0...v0.2.0) (2026-07-31)


### Features

* **ci:** build and attach release binaries ([958bd60](https://github.com/mortennordbye/ruter-cli/commit/958bd60064b09867bed7c8384c2b33217de3bdb4))
