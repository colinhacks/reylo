alter table t drop index f;
alter table t
  drop index k,
  add fulltext key f (b) with parser ngram,
  add key k (a) comment 'new';
