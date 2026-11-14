drop index ti;
drop macro m;
drop index tk;
drop macro k;
create macro k (x) as ((x * 3));
create index tk on t((k(a)));
create macro n (x) as ((x + 3));
create index ti on t((n(a)));
