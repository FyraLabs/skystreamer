# SkyStreamer

SkyStreamer is an AT Firehose consumer that streams new posts from Bluesky. It provides simplified wrapper data types and a
streaming API for easy filtering of events on the AT Firehose.

SkyStreamer is based on [ATrium](https://github.com/sugyan/atrium).

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

By default, SkyStreamer will stream its data into a SurrealDB database. You may change this using environment variables or CLI arguments.

You may also stream the data into a CSV file, or a newline-delimited JSON file (JSONL).

## Older implementation

SkyStreamer was originally implemented as a simple Python script. You can find the old implementation in the `legacy` directory.

## License

SkyStreamer is licensed under the MIT License. See the `LICENSE` file for more information.

This software is provided as-is, without any warranty or guarantee of any kind. Use at your own risk.
Fyra Labs is not responsible for any misuse or damage caused by this software.
