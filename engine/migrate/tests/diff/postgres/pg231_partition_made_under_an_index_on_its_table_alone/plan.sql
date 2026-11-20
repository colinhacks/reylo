-- loses data: p_pd
drop index i;
drop table p_pd;
create table x partition of p for values in (101);
create index i on only p (c);
