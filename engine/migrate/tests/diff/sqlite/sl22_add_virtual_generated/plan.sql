alter table users add column n2 integer generated always as (n * 2) virtual;
