# `webviz`

## TODO

### Scraper

- [x] Basic scraper setup
- [ ] Deal with Wikipedia rate limiting
  - [ ] Look at [`governor`](https://crates.io/crates/governor)
  - [ ] [WikiMedia Rate Limits](https://www.mediawiki.org/wiki/Wikimedia_APIs/Rate_limits)
  - [ ] Re-enqueue or retry jobs that were rate limited.
- [ ] Persistence layer
  - [ ] Can start with JSONL until that borks
  - [ ] Custom binary format?

### Visualizer

- [ ] Bevy graph network
- [ ] [Compressed sparse rows](https://en.wikipedia.org/wiki/Sparse_matrix) for
      the nodes
- [ ] Force-directed layout algorithm
  - [Fruchterman-Reingold](https://en.wikipedia.org/wiki/Force-directed_graph_drawing), $O(n^{2})$
  - Instance rendering

### Putting Together

- [ ] Live streaming from scraper &rarr; Bevy app
