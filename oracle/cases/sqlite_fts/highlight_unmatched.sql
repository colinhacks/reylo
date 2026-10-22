select rowid as id, highlight(words, :col, :open, :close) as hit, bm25(words) as rank from words
