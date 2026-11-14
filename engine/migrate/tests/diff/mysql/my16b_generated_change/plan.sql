alter table users modify column n2 int generated always as ((n * 3)) stored;
