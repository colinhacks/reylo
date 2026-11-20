create table p (id int, c json[]);
create table h (c json[], id int);
alter table h inherit p;
create table pp (id int, n int[]) partition by list (id);
create table pq (n int[], id int);
alter table pp attach partition pq for values in (1);
