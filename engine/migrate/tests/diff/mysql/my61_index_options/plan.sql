alter table t
  drop index f,
  drop index k,
  add fulltext key f (b) with parser ngram,
  add key k (a) comment 'new';
