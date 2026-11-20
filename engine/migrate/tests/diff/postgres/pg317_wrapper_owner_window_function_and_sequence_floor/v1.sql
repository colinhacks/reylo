create foreign data wrapper w;
create function rank_of(x int) returns int language internal as 'window_rank';
create sequence s;
