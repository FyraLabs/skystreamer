# Cappy's Bluesky data analytics workspace

This is my personal workspace for scraping data from the AT (Bluesky) protocol and analyzing them.

Some people might be against the very idea of me doing this, but I don't really care about you. I'm just curious and I want to show you guys
that big data is cool. Ethics is kinda boring anyway.

## What is Bluesky?

Bluesky is a social media platform based on the AT protocol. It's meant to be a decentralized social media platform but it's not really decentralized
since most people just use bsky.social to access it, and barely anyone could not be bothered to run their own node. But it's still cool.

They also *really* hate the fact that someone's been scraping a million posts from their platform. I say try me next.

## Components

- `at_hose.py`: The main script that streams data from the AT network, and saves them into a SurrealDB database.

Other components will be added as I go along.
