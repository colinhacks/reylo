alter table t1 modify column g int generated always as (char_length(_binary'€')) stored;
