create table t (id int primary key, a int, b text, key k (a) comment 'old', fulltext key f (b));
