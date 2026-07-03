# Changelog

## [0.2.0](https://github.com/duartelcunha/ember/compare/v0.1.2...v0.2.0) (2026-07-03)


### Features

* **core:** add monitor_containing for cursor-based monitor lookup ([72315fb](https://github.com/duartelcunha/ember/commit/72315fb321573c2fcdcb0af54f7d5f7189fa5cce))
* **overlay:** shrink orb, retint to Claude terracotta, swap spinner for a pulsing glow ([d045e8b](https://github.com/duartelcunha/ember/commit/d045e8bca0fa1eef363152bf4de43f28ee8697a8))
* **overlay:** shrink the orb further and tint success to the orb accent ([7f3a174](https://github.com/duartelcunha/ember/commit/7f3a174d413e02939bc1372063bfad11f9b665cd))
* stream refine responses with live progress, cancel, and safe capture ([d06f5a8](https://github.com/duartelcunha/ember/commit/d06f5a871369c5ea41ed1ca58fdf0d84cc69ae0b))


### Bug Fixes

* clamp orb to the monitor under the cursor, not the window's stale monitor ([7e7751c](https://github.com/duartelcunha/ember/commit/7e7751c596284596909c9579717ed0a337097933))
* **core:** make the refiner honest about truncation, language, and untrusted input ([6591a3b](https://github.com/duartelcunha/ember/commit/6591a3b1ce0ef05ec68ba2adfbbbd1d806c2818c))
* **core:** remove em-dash from monitor_containing doc comment ([9efc0ff](https://github.com/duartelcunha/ember/commit/9efc0ffc9161d16ef7fc6a9869f31231880f4af0))
* **core:** sanitize loaded config and back up a corrupt file instead of resetting it ([c59b80d](https://github.com/duartelcunha/ember/commit/c59b80d0d4a0def458f72ac600a2e6db03d74bf3))
* **release:** gate builds on tests and hold prerelease until artifacts upload ([51aefad](https://github.com/duartelcunha/ember/commit/51aefad66b62ccae0bdcd6a50193e9bea44933b9))
* **settings:** resolve state-sync bugs, add key removal, and wire accessible labels ([18fca45](https://github.com/duartelcunha/ember/commit/18fca452fb14622a551ab11e66ca256c1e47479f))
* surface Credential Manager failures and extend the terminal list ([de30be4](https://github.com/duartelcunha/ember/commit/de30be4b030878f897a51555bfacf9bd3b18a9a8))

## [0.1.2](https://github.com/duartelcunha/ember/compare/v0.1.1...v0.1.2) (2026-07-01)


### Bug Fixes

* run release-please and the signed build in one workflow ([#2](https://github.com/duartelcunha/ember/issues/2)) ([c004f63](https://github.com/duartelcunha/ember/commit/c004f633140f91874b8398a1d4c761f98ee7eedc))

## [0.1.1](https://github.com/duartelcunha/ember/compare/v0.1.0...v0.1.1) (2026-07-01)


### Bug Fixes

* keep release tags as plain vX.Y.Z (no component prefix) ([965a96e](https://github.com/duartelcunha/ember/commit/965a96ef5822c64d50a1772af5d0be8bbea56793))
* set releaseName so the release workflow can create a new GitHub release ([01b089f](https://github.com/duartelcunha/ember/commit/01b089f6d256a4e55ed402b8265884df6ebfcee2))
