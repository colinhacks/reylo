create table p (id int not null, n text) partition by range (id);
create table p_pd partition of p default;
create table x (id int not null);
