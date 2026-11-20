create table "t x" (id int not null, constraint "t x_pkey" primary key (id)) partition by list (id);
create table "t x_pd" partition of "t x" default;
create table "t x_p0" (id int not null, constraint "t x_p0_pkey" primary key (id));
