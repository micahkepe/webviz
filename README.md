# `webviz`

## TODO

### Scraper

- [x] Basic scraper setup
- [ ] Deal with Wikipedia rate limiting
  - [ ] Look at [`governor`](https://crates.io/crates/governor)
  - [ ] [WikiMedia Rate Limits](https://www.mediawiki.org/wiki/Wikimedia_APIs/Rate_limits)
  - [ ] Re-enqueue or retry jobs that were rate limited.
- [ ] Persistence layer
  - [ ] Custom binary format?

### Visualizer

- [ ] Bevy graph network
- [ ] [Compressed sparse rows](https://en.wikipedia.org/wiki/Sparse_matrix) for
      the nodes
