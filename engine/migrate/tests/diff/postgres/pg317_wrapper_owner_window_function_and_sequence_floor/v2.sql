create foreign data wrapper w;
alter foreign data wrapper w owner to diff_super;
create function rank_of(x int) returns int window language internal as 'window_rank';
create sequence s minvalue 10;
alter default privileges grant select on large objects to diff_owner;
