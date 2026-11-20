create table t1 (id int primary key, g int as (char_length('€')) stored);
