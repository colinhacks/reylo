create table p (id int not null, c int) partition by list (id);
create table p_pd partition of p default;
create index i on only p (c);
