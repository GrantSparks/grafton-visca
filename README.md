# grafton-visca

Rust based VISCA over IP implementation for controlling PTZ Cameras

Currently only PTZOptics G2 VISCA over IP Commands are implemented but it might very well work with other cameras that use the VISCA protocol.  If there is interest we could abstract the commands to make it easier to add other camera types.

** Impotant Note: This is a work in progress and is not yet ready for production use. I am only validating the byte sequences as I use the commands so many have not been checked against the documentation. **

## Installation

Add the following to `Cargo.toml` under `[dependencies]`:

```
grafton-visca = "*"
```

## Contributing

Contributions are welcome! Please submit a pull request or open an issue to discuss what you would like to change.

## About

This is a project by the [Grafton Machine Shed](https://www.grafton.ai)

## License

This project is licensed under the Apache License, Version 2.0. See the [LICENSE](LICENSE) file for more details.
