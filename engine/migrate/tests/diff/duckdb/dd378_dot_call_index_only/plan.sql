drop index ti;
drop macro m;
create macro m (x) as ((x + 2));
create index ti on t((a.m()));
