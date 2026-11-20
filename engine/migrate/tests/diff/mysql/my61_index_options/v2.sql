create table t (id int primary key, a int, b text, key k (a) comment 'new', fulltext key f (b) with parser ngram);
