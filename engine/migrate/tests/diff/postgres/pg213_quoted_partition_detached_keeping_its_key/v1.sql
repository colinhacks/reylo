create table "t x" (id int not null, constraint "t x_pkey" primary key (id)) partition by list (id);
create table "t x_p0" partition of "t x" for values in (1, 2);
create table "t x_pd" partition of "t x" default;
