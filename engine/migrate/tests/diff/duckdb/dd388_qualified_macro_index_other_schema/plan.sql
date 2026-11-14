drop index ti;
drop macro s.f;
create macro s.f (x) as ((x * 3));
create index ti on t((s.f(a)));
