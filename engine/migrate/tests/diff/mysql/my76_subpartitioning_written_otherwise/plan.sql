alter table t2 modify column g int generated always as ((C + 1)) stored;
