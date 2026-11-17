# Optional hardware smoke checks

Hardware smoke checks are optional for stable releases and prereleases. They
are not a publication gate and require no public checklist or sign-off (#1008).

When useful, try a command and inquiry on an available camera, then exercise
the affected transport or motion path with a safe operating area and STOP
controls available. Stop if the device behaves unexpectedly.

Keep any local notes or captures private. Do not add device identifiers,
network addresses, firmware inventories, transcripts or operator records to
release notes or repository documentation.

Describe software support through source-backed profile grants and automated
protocol tests. A private smoke check does not establish a blanket hardware
verification claim. The required software, downstream, tag, CI and package
validation remains in [RELEASING.md](../RELEASING.md).
