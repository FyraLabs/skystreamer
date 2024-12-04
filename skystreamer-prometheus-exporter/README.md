# SkyStreamer Prometheus Exporter

This is a simple implementation of a Prometheus exporter for SkyStreamer analytics. It exports a single `posts` metric that counts the number of posts collected by SkyStreamer.

The counter loops back to 0 when the counter reaches 10000.

To query using Prometheus, use the following query:

```promql
irate(skystreamer_bsky_posts[1m])
```
