# SkyStreamer

SkyStreamer is an AT Firehose consumer that streams new posts from Bluesky. It provides simplified wrapper data types and a
streaming API for easy filtering of events on the AT Firehose, based on [ATrium](https://github.com/sugyan/atrium).

SkyStreamer is part of Project Zenith, an experimental social media analytics project by Fyra Labs.

## Why?

The AT protocol firehose can be a very useful source of data, one may want to consume it in a more structured way.
SkyStreamer helps filter out the noise and only stream new Bluesky posts from the firehose.

## Planned Features

- [x] Make data types non-dependent on SurrealDB while maintaining data types
- [x] Export a crate for easy integration with other projects
- [ ] SSL-independent implementation (Rustls/OpenSSL agnostic)
- [ ] Optimized, multiple-threaded implementation
- [x] Configuration

## Ethics

SkyStreamer collects large amounts of new data streaming from Bluesky itself, which then can be used for many various purposes.

While the network and the data itself is **visibly public** to **everyone**, Some users may be uncomfortable with their data being collected and used in this way, especially for machine learning or similar purposes.

> [!IMPORTANT]
> Please be mindful of the data you are collecting and how you are using it, and respect their consent if possible.
>
> SkyStreamer comes with a [streaming API](https://docs.rs/futures/latest/futures/stream/index.html) that can be used alongside Rust's iterator API, which can be used to filter out data.
> However, it does ***not*** filter or redact any data by default, you will be responsible for your own data processing.
>
> Fyra Labs is not responsible for any misuse of the data collected by SkyStreamer. You are responsible for your own actions.

## Usage

SkyStreamer can be used as a library or as a CLI. The library provides a Tokio stream that can be used to stream data from the AT Firehose.

### As a CLI

By default, SkyStreamer will stream its data into a SurrealDB database. You may change this using environment variables or CLI arguments.

You may also stream the data into an [IETF RFC 4180](https://www.ietf.org/rfc/rfc4180.txt) CSV-formatted file, or log it into the console.

```shell
skystreamer -E csv -o data.csv
```

See `skystreamer --help` for more information.

### As a library

Please see the `skystreamer/examples` directory for examples on how to use SkyStreamer as a library.

### Prometheus Exporter

SkyStreamer also has a Prometheus exporter implementation that can be used to show statistics about Bluesky posts as a whole.

See [skystreamer-prometheus-exporter](./skystreamer-prometheus-exporter) for the implentation.

You can also use the Docker image from `ghcr.io/fyralabs/skystreamer-prometheus-exporter:latest` to run the exporter. Configure it like you would
normally configure Prometheus data sources.

The data itself only includes numbers of posts and various counters per metric, and does not include any actual post data except for external link domains and hashtags.

#### Environment Variables

- `MAX_SAMPLE_SIZE`: Maximum number of posts to count before overflow, default is none (Max `u64`).
- `NORMALIZE_LANGS`: Attempt to normalize post language codes to their respective [IETF BCP 47](https://www.ietf.org/rfc/bcp/bcp47.html) codes, default is `true`. Set to `false` to export raw codes from the AT firehose.

## Older implementation

SkyStreamer was originally implemented as a simple Python script. You can find the old implementation in the `legacy` directory.

## License

SkyStreamer is licensed under the MIT License. See the `LICENSE` file for more information.

This software is provided as-is, without any warranty or guarantee of any kind. Use at your own risk.
Fyra Labs is not responsible for any misuse or damage caused by this software.
