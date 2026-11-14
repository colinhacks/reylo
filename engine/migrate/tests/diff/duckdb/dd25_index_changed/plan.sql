drop index t_ab;
create index t_ab on t(a, b);
create unique index t_b on t(b);
